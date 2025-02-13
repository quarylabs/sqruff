use ahash::AHashMap;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::parser::segments::fix::SourceFix;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Aliasing {
    Explicit,
    Implicit,
}

#[derive(Debug, Clone)]
pub struct RuleJJ01;

impl Rule for RuleJJ01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleJJ01.erased())
    }

    fn name(&self) -> &'static str {
        "jinja.padding"
    }

    fn description(&self) -> &'static str {
        "Jinja tags should have a single whitespace on either side."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Jinja tags with either no whitespace or very long whitespace are hard to read.

```sql
SELECT {{    a     }} from {{ref('foo')}}
```

**Best practice**

A single whitespace surrounding Jinja tags, alternatively longer gaps containing newlines are acceptable.

```sql
SELECT {{ a }} from {{ ref('foo') }};
SELECT {{ a }} from {{
     ref('foo')
}};
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Jinja]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        debug_assert!(context.segment.get_position_marker().is_some());

        // If the position marker for the root segment is literal then there's
        // no templated code, so return early
        if context.segment.get_position_marker().unwrap().is_literal() {
            return vec![];
        }

        let mut results: Vec<LintResult> = vec![];

        // Work through the templated slices
        for raw_slice in context.templated_file.raw_sliced_iter() {
            // Only want templated slices
            if !matches!(
                raw_slice.slice_type.as_str(),
                "templated" | "block_start" | "block_end"
            ) {
                continue;
            }

            let stripped = raw_slice.raw.trim();
            if stripped.is_empty() || !stripped.starts_with('{') || !stripped.ends_with('}') {
                continue;
            }

            // Partition and position
            let src_idx = raw_slice.source_idx;
            let (tag_pre, ws_pre, inner, ws_post, tag_post) =
                Self::get_white_space_ends(stripped.to_string());

            let position = raw_slice
                .raw
                .find(stripped.chars().next().unwrap())
                .unwrap_or(0);

            // Whitespace should be single space OR contain newline
            let mut pre_fix = None;
            let mut post_fix = None;

            if ws_pre.is_empty() || (ws_pre != " " && !ws_pre.contains('\n')) {
                pre_fix = Some(" ");
            }
            if ws_post.is_empty() || (ws_post != " " && !ws_post.contains('\n')) {
                post_fix = Some(" ");
            }

            // Skip if no fixes needed
            if pre_fix.is_none() && post_fix.is_none() {
                continue;
            }

            let fixed = format!(
                "{}{}{}{}{}",
                tag_pre,
                pre_fix.unwrap_or(&ws_pre),
                inner,
                post_fix.unwrap_or(&ws_post),
                tag_post
            );

            // Find raw segment to attach fix to
            let Some(raw_seg) = Self::find_raw_at_src_index(&context.segment, src_idx)
            else {
                continue;
            };

            // Skip if segment already has fixes
            if !raw_seg.get_source_fixes().is_empty() {
                continue;
            }

            let ps_marker = raw_seg
                .get_position_marker()
                .map(|pm| pm.templated_slice.clone());
            let Some(ps_marker) = ps_marker else {
                continue;
            };

            let source_fixes = vec![SourceFix::new(
                fixed.clone().into(),
                src_idx + position..src_idx + position + stripped.len(),
                ps_marker,
            )];

            results.push(LintResult::new(
                Some(raw_seg.clone()),
                vec![LintFix::replace(
                    raw_seg.clone(),
                    vec![raw_seg.edit(raw_seg.id(), fixed.into(), Some(source_fixes))],
                    None,
                )],
                Some(format!(
                    "Jinja tags should have a single whitespace on either side: {}",
                    stripped
                )),
                None,
            ));
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

impl RuleJJ01 {
    fn get_white_space_ends(s: String) -> (String, String, String, String, String) {
        assert!(
            s.starts_with('{') && s.ends_with('}'),
            "String must start with {{ and end with }}"
        );

        // Get the main content between the tag markers
        let mut main = s[2..s.len() - 2].to_string();
        let mut pre = s[..2].to_string();
        let mut post = s[s.len() - 2..].to_string();

        // Handle plus/minus modifiers
        let modifier_chars = ['+', '-'];
        if !main.is_empty() && modifier_chars.contains(&main.chars().next().unwrap()) {
            let first_char = main.chars().next().unwrap();
            main = main[1..].to_string();
            // Keep the modifier directly after {% or {{
            pre = format!("{}{}", pre, first_char);
        }
        if !main.is_empty() && modifier_chars.contains(&main.chars().last().unwrap()) {
            let last_char = main.chars().last().unwrap();
            main = main[..main.len() - 1].to_string();
            // Keep the modifier directly before %} or }}
            post = format!("{}{}", last_char, post);
        }

        // Split out inner content and surrounding whitespace
        let inner = main.trim().to_string();
        let pos = main.find(&inner).unwrap_or(0);
        let pre_ws = main[..pos].to_string();
        let post_ws = main[pos + inner.len()..].to_string();

        (pre, pre_ws, inner, post_ws, post)
    }

    fn find_raw_at_src_index(segment: &ErasedSegment, src_idx: usize) -> Option<&ErasedSegment> {
        // Recursively search to find a raw segment for a position in the source.
        // NOTE: This assumes it's not being called on a `raw`.
        // In the case that there are multiple potential targets, we will find the first.
        assert!(!segment.is_raw(), "Segment must not be raw");
        let segments = segment.segments();
        assert!(segments.len() > 0, "Segment must have segments");

        for seg in segments {
            let Some(pos_marker) = seg.get_position_marker() else {
                continue;
            };
            // If it's before, skip onward
            if pos_marker.source_slice.end <= src_idx {
                continue;
            }
            // Is the current segment raw?
            if seg.is_raw() {
                return Some(seg);
            }
            // Otherwise recurse
            return Self::find_raw_at_src_index(seg, src_idx);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{config::FluffConfig, linter::core::Linter},
        templaters::jinja::JinjaTemplater,
    };

    #[test]
    fn test_get_white_space_ends() {
        let cases = vec![
            (
                "{{+ my_content }}",
                (
                    "{{+".to_string(),
                    " ".to_string(),
                    "my_content".to_string(),
                    " ".to_string(),
                    "}}".to_string(),
                ),
            ),
            (
                "{%+if true-%}",
                (
                    "{%+".to_string(),
                    "".to_string(),
                    "if true".to_string(),
                    "".to_string(),
                    "-%}".to_string(),
                ),
            ),
        ];

        for (input, expected) in cases {
            let result = RuleJJ01::get_white_space_ends(input.to_string());
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_simple_example() {
        let start = "SELECT 1 from {%+if true-%} {{ref('foo')}} {%-endif%}".to_string();
        let want = "SELECT 1 from {%+ if true -%} {{ ref('foo') }} {%- endif %}".to_string();

        let config = FluffConfig::from_source(
            r#"
[sqruff]
rules = JJ01 
templater = jinja
            "#,
            None,
        );

        let mut linter = Linter::new(config, None, Some(&JinjaTemplater), false);
        let result = linter.lint_string_wrapped(&start, None, true);

        let fixed = result.paths[0].files[0].clone().fix_string();
        assert_eq!(fixed, want, "\nExpected: {}\nGot: {}", want, fixed);
    }
}
