use ahash::AHashMap;
use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::SegmentBuilder;
use strum_macros::{AsRefStr, EnumString};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

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
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
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
        // TODO: "databricks", "hive", "mysql"
        if !(self.force_enable
            || matches!(
                context.dialect.name,
                DialectKind::Bigquery | DialectKind::Sparksql
            ))
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
            return vec![LintResult::new(
                context.segment.clone().into(),
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
                )],
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
        new_body = format!("{}\\{}", truncated_body, preferred_quote_char);
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
