use ahash::AHashMap;
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::IndexMap;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
pub struct RuleLT08;

impl Rule for RuleLT08 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT08.erased())
    }
    fn name(&self) -> &'static str {
        "layout.cte_newline"
    }

    fn description(&self) -> &'static str {
        "Blank line expected but not found after CTE closing bracket."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

There is no blank line after the CTE closing bracket. In queries with many CTEs, this hinders readability.

```sql
WITH plop AS (
    SELECT * FROM foo
)
SELECT a FROM plop
```

**Best practice**

Add a blank line.

```sql
WITH plop AS (
    SELECT * FROM foo
)

SELECT a FROM plop
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut error_buffer = Vec::new();
        let global_comma_style = context.config.raw["layout"]["type"]["comma"]["line_position"]
            .as_string()
            .unwrap();
        let expanded_segments = context.segment.iter_segments(
            const { &SyntaxSet::new(&[SyntaxKind::CommonTableExpression]) },
            false,
        );

        let bracket_indices = expanded_segments
            .iter()
            .enumerate()
            .filter_map(|(idx, seg)| seg.is_type(SyntaxKind::Bracketed).then_some(idx));

        for bracket_idx in bracket_indices {
            let forward_slice = &expanded_segments[bracket_idx..];
            let mut seg_idx = 1;
            let mut line_idx: usize = 0;
            let mut comma_seg_idx = 0;
            let mut blank_lines = 0;
            let mut comma_line_idx = None;
            let mut line_blank = false;
            let mut line_starts = IndexMap::default();
            let mut comment_lines = Vec::new();

            while forward_slice[seg_idx].is_type(SyntaxKind::Comma)
                || !forward_slice[seg_idx].is_code()
            {
                if forward_slice[seg_idx].is_type(SyntaxKind::Newline) {
                    if line_blank {
                        // It's a blank line!
                        blank_lines += 1;
                    }
                    line_blank = true;
                    line_idx += 1;
                    line_starts.insert(line_idx, seg_idx + 1);
                } else if forward_slice[seg_idx].is_type(SyntaxKind::Comment)
                    || forward_slice[seg_idx].is_type(SyntaxKind::InlineComment)
                    || forward_slice[seg_idx].is_type(SyntaxKind::BlockComment)
                {
                    // Lines with comments aren't blank
                    line_blank = false;
                    comment_lines.push(line_idx);
                } else if forward_slice[seg_idx].is_type(SyntaxKind::Comma) {
                    // Keep track of where the comma is.
                    // We'll evaluate it later.
                    comma_line_idx = line_idx.into();
                    comma_seg_idx = seg_idx;
                }

                seg_idx += 1;
            }

            let comma_style = if comma_line_idx.is_none() {
                "final"
            } else if line_idx == 0 {
                "oneline"
            } else if let Some(0) = comma_line_idx {
                "trailing"
            } else if let Some(idx) = comma_line_idx {
                if idx == line_idx {
                    "leading"
                } else {
                    "floating"
                }
            } else {
                "floating"
            };

            if blank_lines >= 1 {
                continue;
            }

            let mut is_replace = false;
            let mut fix_point = None;

            let num_newlines = if comma_style == "oneline" {
                if global_comma_style == "trailing" {
                    fix_point = forward_slice[comma_seg_idx + 1].clone().into();
                    if forward_slice[comma_seg_idx + 1].is_type(SyntaxKind::Whitespace) {
                        is_replace = true;
                    }
                } else if global_comma_style == "leading" {
                    fix_point = forward_slice[comma_seg_idx].clone().into();
                } else {
                    unimplemented!("Unexpected global comma style {global_comma_style:?}");
                }

                2
            } else {
                if comma_style == "leading" {
                    if comma_seg_idx < forward_slice.len() {
                        fix_point = forward_slice[comma_seg_idx].clone().into();
                    }
                } else if comment_lines.is_empty() || !comment_lines.contains(&(line_idx - 1)) {
                    if matches!(comma_style, "trailing" | "final" | "floating") {
                        if forward_slice[seg_idx - 1].is_type(SyntaxKind::Whitespace) {
                            fix_point = forward_slice[seg_idx - 1].clone().into();
                            is_replace = true;
                        } else {
                            fix_point = forward_slice[seg_idx].clone().into();
                        }
                    }
                } else {
                    let mut offset = 1;

                    while line_idx
                        .checked_sub(offset)
                        .is_some_and(|idx| comment_lines.contains(&idx))
                    {
                        offset += 1;
                    }

                    let mut effective_line_idx = line_idx - (offset - 1);
                    if effective_line_idx == 0 {
                        effective_line_idx = line_idx;
                    }

                    let line_start_idx = if effective_line_idx < line_starts.len() {
                        *line_starts.get(&effective_line_idx).unwrap()
                    } else {
                        let (_, line_start) = line_starts.last().unwrap_or((&0, &0));
                        *line_start
                    };

                    fix_point = forward_slice[line_start_idx].clone().into();
                }

                1
            };

            // Only create fixes if we have a valid fix point
            let fixes = if let Some(anchor) = fix_point {
                let newlines = std::iter::repeat_n(
                    SegmentBuilder::newline(context.tables.next_id(), "\n"),
                    num_newlines,
                )
                .collect_vec();

                if is_replace {
                    vec![LintFix::replace(anchor, newlines, None)]
                } else {
                    vec![LintFix::create_before(anchor, newlines)]
                }
            } else {
                // Skip generating a fix if we don't have a valid anchor point
                Vec::new()
            };

            error_buffer.push(LintResult::new(
                forward_slice[seg_idx].clone().into(),
                fixes,
                None,
                None,
            ));
        }

        error_buffer
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::WithCompoundStatement]) })
            .into()
    }
}
