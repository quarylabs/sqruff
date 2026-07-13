use std::ops::Range;

use hashbrown::HashMap;
use regex::Regex;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::markers::PositionMarker;
use sqruff_lib_core::parser::segments::SegmentBuilder;
use sqruff_lib_core::parser::segments::fix::SourceFix;
use sqruff_lib_core::templaters::TemplateSliceKind;
use strum_macros::{AsRefStr, EnumString};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups, targets_templated};

#[derive(Debug, Copy, Clone, AsRefStr, EnumString, PartialEq, Default)]
#[strum(serialize_all = "snake_case")]
enum PreferredQuotedLiteralStyle {
    #[default]
    Consistent,
    SingleQuotes,
    DoubleQuotes,
}

impl PreferredQuotedLiteralStyle {
    fn info(&self) -> QuoteInfo {
        match self {
            PreferredQuotedLiteralStyle::Consistent => unimplemented!(),
            PreferredQuotedLiteralStyle::SingleQuotes => QuoteInfo {
                preferred_quote_char: '\'',
                alternate_quote_char: '"',
            },
            PreferredQuotedLiteralStyle::DoubleQuotes => QuoteInfo {
                preferred_quote_char: '"',
                alternate_quote_char: '\'',
            },
        }
    }
}

struct QuoteInfo {
    preferred_quote_char: char,
    alternate_quote_char: char,
}

#[derive(Clone, Debug, Default)]
pub struct RuleCV10 {
    preferred_quoted_literal_style: PreferredQuotedLiteralStyle,
    force_enable: bool,
}

impl Rule for RuleCV10 {
    targets_templated!();

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV10 {
            preferred_quoted_literal_style: config["preferred_quoted_literal_style"]
                .as_string()
                .unwrap()
                .to_owned()
                .parse()
                .unwrap(),
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.quoted_literals"
    }

    fn description(&self) -> &'static str {
        "Consistent usage of preferred quotes for quoted literals."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

```sql
select
    "abc",
    'abc',
    "\"",
    "abc" = 'abc'
from foo
```

**Best practice**

Ensure all quoted literals use preferred quotes, unless escaping can be reduced by using alternate quotes.

```sql
select
    "abc",
    "abc",
    '"',
    "abc" = "abc"
from foo
```P        
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // TODO: "databricks", "mysql"
        if !(self.force_enable
            || matches!(
                context.dialect.name,
                DialectKind::Bigquery | DialectKind::Hive | DialectKind::Sparksql
            ))
        {
            return Vec::new();
        }

        let Some(position_marker) = context.segment.get_position_marker() else {
            return Vec::new();
        };
        let spans_template = segment_spans_templated_slice(position_marker);
        if spans_template
            && !quote_delimiters_are_in_literal_source(
                context.segment.raw().as_ref(),
                position_marker,
            )
        {
            return Vec::new();
        }

        let preferred_quoted_literal_style =
            if self.preferred_quoted_literal_style == PreferredQuotedLiteralStyle::Consistent {
                let preferred_quoted_literal_style = context
                    .try_get::<PreferredQuotedLiteralStyle>()
                    .unwrap_or_else(|| {
                        if context.segment.raw().ends_with('"') {
                            PreferredQuotedLiteralStyle::DoubleQuotes
                        } else {
                            PreferredQuotedLiteralStyle::SingleQuotes
                        }
                    });

                context.set(preferred_quoted_literal_style);
                preferred_quoted_literal_style
            } else {
                self.preferred_quoted_literal_style
            };

        let info = preferred_quoted_literal_style.info();
        let fixed_string = normalize_preferred_quoted_literal_style(
            context.segment.raw().as_ref(),
            info.preferred_quote_char,
            info.alternate_quote_char,
        );

        if fixed_string != context.segment.raw().as_str() {
            let fixes = if spans_template {
                source_only_quote_fixes(context, &fixed_string)
                    .map(|source_fixes| {
                        let raw_token = SegmentBuilder::token(
                            context.tables.next_id(),
                            context.segment.raw().as_ref(),
                            context.segment.get_type(),
                        )
                        .finish();

                        let edit_segment = SegmentBuilder::node(
                            context.tables.next_id(),
                            context.segment.get_type(),
                            context.dialect.name,
                            vec![raw_token],
                        )
                        .with_source_fixes(source_fixes)
                        .finish();

                        vec![LintFix::replace(
                            context.segment.clone(),
                            vec![edit_segment],
                            None,
                        )]
                    })
                    .unwrap_or_default()
            } else {
                vec![LintFix::replace(
                    context.segment.clone(),
                    vec![
                        SegmentBuilder::token(
                            context.tables.next_id(),
                            &fixed_string,
                            SyntaxKind::QuotedLiteral,
                        )
                        .finish(),
                    ],
                    None,
                )]
            };

            return vec![LintResult::new(
                context.segment.clone().into(),
                fixes,
                Some("".into()),
                None,
            )];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::QuotedLiteral]) }).into()
    }
}

fn segment_spans_templated_slice(position_marker: &PositionMarker) -> bool {
    position_marker
        .templated_file
        .raw_sliced()
        .iter()
        .any(|slice| {
            slice.has_slice_kind(TemplateSliceKind::Templated)
                && ranges_overlap(&slice.source_slice(), &position_marker.source_slice).is_some()
        })
}

fn quote_delimiters_are_in_literal_source(raw: &str, position_marker: &PositionMarker) -> bool {
    let Some((leading_offset, leading_quote)) = raw
        .char_indices()
        .find(|(_, ch)| !matches!(ch, 'r' | 'b' | 'R' | 'B'))
    else {
        return false;
    };
    let Some((trailing_offset, trailing_quote)) = raw.char_indices().next_back() else {
        return false;
    };

    matches!(leading_quote, '\'' | '"')
        && matches!(trailing_quote, '\'' | '"')
        && templated_position_is_literal_source(
            position_marker,
            position_marker.templated_slice.start + leading_offset,
            leading_quote,
        )
        && templated_position_is_literal_source(
            position_marker,
            position_marker.templated_slice.start + trailing_offset,
            trailing_quote,
        )
}

fn templated_position_is_literal_source(
    position_marker: &PositionMarker,
    templated_pos: usize,
    expected: char,
) -> bool {
    position_marker
        .templated_file
        .sliced_file
        .iter()
        .find(|slice| {
            slice.templated_slice.start <= templated_pos
                && templated_pos < slice.templated_slice.end
        })
        .is_some_and(|slice| {
            if !slice.has_slice_kind(TemplateSliceKind::Literal) {
                return false;
            }

            let source_pos = slice.source_slice.start + templated_pos - slice.templated_slice.start;
            position_marker.templated_file.source_str[source_pos..].starts_with(expected)
        })
}

fn source_only_quote_fixes(context: &RuleContext, fixed_string: &str) -> Option<Vec<SourceFix>> {
    let position_marker = context.segment.get_position_marker()?;
    let raw = context.segment.raw();

    if raw.len() != fixed_string.len() {
        return None;
    }

    let mut source_fixes = Vec::new();
    let templated_file = &position_marker.templated_file;

    for slice in &templated_file.sliced_file {
        let Some(overlap) =
            ranges_overlap(&slice.templated_slice, &position_marker.templated_slice)
        else {
            continue;
        };
        if overlap.is_empty() {
            continue;
        }

        let local_slice = overlap.start - position_marker.templated_slice.start
            ..overlap.end - position_marker.templated_slice.start;
        let raw_part = raw.get(local_slice.clone())?;
        let fixed_part = fixed_string.get(local_slice.clone())?;

        if !slice.has_slice_kind(TemplateSliceKind::Literal) {
            if raw_part != fixed_part {
                return None;
            }
            continue;
        }

        if raw_part == fixed_part {
            continue;
        }

        let source_start = slice.source_slice.start + overlap.start - slice.templated_slice.start;
        let source_end = source_start + overlap.end - overlap.start;
        source_fixes.push(SourceFix::new(
            SmolStr::new(fixed_part),
            source_start..source_end,
            overlap,
        ));
    }

    if source_fixes.is_empty() {
        None
    } else {
        Some(source_fixes)
    }
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
}

// FIXME: avoid memory allocations
fn normalize_preferred_quoted_literal_style(
    s: &str,
    preferred_quote_char: char,
    alternate_quote_char: char,
) -> String {
    let mut s = s.to_string();
    let trimmed = s.trim_start_matches(['r', 'b', 'R', 'B']);

    let (orig_quote, new_quote) = if trimmed
        .chars()
        .take(3)
        .eq(std::iter::repeat_n(preferred_quote_char, 3))
    {
        return s.to_string();
    } else if trimmed.starts_with(preferred_quote_char) {
        (
            preferred_quote_char.to_string(),
            alternate_quote_char.to_string(),
        )
    } else if trimmed
        .chars()
        .take(3)
        .eq(std::iter::repeat_n(alternate_quote_char, 3))
    {
        (
            std::iter::repeat_n(alternate_quote_char, 3).collect(),
            std::iter::repeat_n(preferred_quote_char, 3).collect(),
        )
    } else if trimmed.starts_with(alternate_quote_char) {
        (
            alternate_quote_char.to_string(),
            preferred_quote_char.to_string(),
        )
    } else {
        return s.to_string();
    };

    let first_quote_pos = s.find(&orig_quote).unwrap_or_default();
    let prefix = s[..first_quote_pos].to_string();
    let unescaped_new_quote = Regex::new(&format!(r"(([^\\]|^)(\\\\)*){new_quote}")).unwrap();
    let escaped_new_quote = Regex::new(&format!(r"([^\\]|^)\\((?:\\\\)*){new_quote}")).unwrap();
    let escaped_orig_quote = Regex::new(&format!(r"([^\\]|^)\\((?:\\\\)*){orig_quote}")).unwrap();

    let body_start = first_quote_pos + orig_quote.len();
    let body_end = s.len() - orig_quote.len();

    let mut body = s[body_start..body_end].to_string();
    let mut new_body = if prefix.to_lowercase().contains("r") {
        if unescaped_new_quote.find(&body).is_some() {
            return s.to_string();
        }
        body.clone()
    } else {
        let mut new_body =
            regex_sub_with_overlap(&escaped_new_quote, &format!(r"$1$2{new_quote}"), &body);
        if new_body != body {
            body = new_body.clone();
            s = format!("{prefix}{orig_quote}{body}{orig_quote}");
        }
        new_body = regex_sub_with_overlap(
            &escaped_orig_quote,
            &format!(r"$1$2{orig_quote}"),
            &new_body,
        );
        new_body = regex_sub_with_overlap(
            &unescaped_new_quote,
            &format!(r"$1\\{new_quote}"),
            &new_body,
        );

        new_body
    };

    if new_quote
        .chars()
        .eq(std::iter::repeat_n(preferred_quote_char, 3))
        && new_body.ends_with(preferred_quote_char)
    {
        let truncated_body = &new_body[..new_body.len() - 1];
        new_body = format!("{truncated_body}\\{preferred_quote_char}");
    }

    let orig_escape_count = body.matches("\\").count();
    let new_escape_count = new_body.matches("\\").count();
    if new_escape_count > orig_escape_count {
        return s.to_string();
    }

    if new_escape_count == orig_escape_count && orig_quote.starts_with(preferred_quote_char) {
        s.to_string()
    } else {
        format!("{prefix}{new_quote}{new_body}{new_quote}")
    }
}

fn regex_sub_with_overlap(regex: &Regex, replacement: &str, original: &str) -> String {
    let first_pass = regex.replace_all(original, replacement);
    let second_pass = regex.replace_all(&first_pass, replacement);
    second_pass.to_string()
}
