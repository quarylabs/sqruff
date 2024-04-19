use std::iter::repeat;

use ahash::AHashMap;
use indexmap::IndexMap;
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::parser::segments::base::NewlineSegment;
use crate::core::rules::base::{EditType, Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default, Clone)]
pub struct RuleLT08 {}

impl Rule for RuleLT08 {
    fn from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleLT08::default().erased()
    }

    fn name(&self) -> &'static str {
        "layout.cte_newline"
    }

    fn description(&self) -> &'static str {
        "Blank line expected but not found after CTE closing bracket."
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["with_compound_statement"].into()).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut error_buffer = Vec::new();
        let global_comma_style = "trailing";
        let expanded_segments =
            context.segment.iter_segments(["common_table_expression"].as_slice().into(), false);

        let bracket_indices = expanded_segments
            .iter()
            .enumerate()
            .filter_map(|(idx, seg)| seg.is_type("bracketed").then_some(idx));

        for bracket_idx in bracket_indices {
            let forward_slice = &expanded_segments[bracket_idx..];
            let mut seg_idx = 1;
            let mut line_idx: usize = 0;
            let mut comma_seg_idx = 0;
            let mut blank_lines = 0;
            let mut comma_line_idx = None;
            let mut line_blank = false;
            let mut line_starts = IndexMap::new();
            let mut comment_lines = Vec::new();

            while forward_slice[seg_idx].is_type("comma") || !forward_slice[seg_idx].is_code() {
                if forward_slice[seg_idx].is_type("newline") {
                    if line_blank {
                        // It's a blank line!
                        blank_lines += 1;
                    }
                    line_blank = true;
                    line_idx += 1;
                    line_starts.insert(line_idx, seg_idx + 1);
                } else if forward_slice[seg_idx].is_type("comment") {
                    // Lines with comments aren't blank
                    line_blank = false;
                    comment_lines.push(line_idx);
                } else if forward_slice[seg_idx].is_type("comma") {
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
                if idx == line_idx { "leading" } else { "floating" }
            } else {
                "floating"
            };

            if blank_lines >= 1 {
                continue;
            }

            let mut fix_type = EditType::CreateBefore;
            let mut fix_point = None;

            let num_newlines = if comma_style == "oneline" {
                if global_comma_style == "trailing" {
                    fix_point = forward_slice[comma_seg_idx + 1].clone().into();
                    if forward_slice[comma_seg_idx + 1].is_type("whitespace") {
                        fix_type = EditType::Replace;
                    }
                } else if global_comma_style == "leading" {
                    fix_point = forward_slice[comma_seg_idx].clone().into();
                } else {
                    unimplemented!("Unexpected global comma style {global_comma_style:?}");
                }

                2
            } else {
                if comment_lines.is_empty() || !comment_lines.contains(&(line_idx - 1)) {
                    if matches!(comma_style, "trailing" | "final" | "floating") {
                        if forward_slice[seg_idx - 1].is_type("whitespace") {
                            fix_point = forward_slice[seg_idx - 1].clone().into();
                            fix_type = EditType::Replace;
                        } else {
                            fix_point = forward_slice[seg_idx].clone().into();
                        }
                    }
                } else if comma_style == "leading" {
                    fix_point = forward_slice[comma_seg_idx].clone().into();
                } else {
                    let mut offset = 1;

                    while line_idx
                        .checked_sub(offset)
                        .map_or(false, |idx| comment_lines.contains(&idx))
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

            let fixes = vec![LintFix {
                edit_type: fix_type,
                anchor: fix_point.unwrap(),
                edit: repeat(NewlineSegment::create("\n", &<_>::default(), <_>::default()))
                    .take(num_newlines)
                    .collect_vec()
                    .into(),
                source: Vec::new(),
            }];

            error_buffer.push(LintResult::new(
                forward_slice[seg_idx].clone().into(),
                fixes,
                None,
                None,
                None,
            ));
        }

        error_buffer
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT08::RuleLT08;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT08::default().erased()]
    }

    #[test]
    fn test_pass_blank_line_after_cte_trailing_comma() {
        let sql = "
        with my_cte as (
            select 1
        ),

        other_cte as (
            select 1
        )

        select * from my_cte cross join other_cte
    ";

        let lints = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_pass_blank_line_after_cte_leading_comma() {
        let sql = "
        with my_cte as (
            select 1
        )

        , other_cte as (
            select 1
        )

        select * from my_cte cross join other_cte
    ";

        let lints = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(lints, []);
    }

    #[test]
    fn test_fail_no_blank_line_after_each_cte() {
        let sql = "
with my_cte as (
    select 1
),
other_cte as (
    select 1
)

select * from my_cte cross join other_cte";

        let fixed = fix(sql.into(), rules());
        assert_eq!(
            fixed,
            "
with my_cte as (
    select 1
),

other_cte as (
    select 1
)

select * from my_cte cross join other_cte"
        );
    }

    #[test]
    fn test_fail_no_blank_line_after_cte_before_comment() {
        let sql = "
with my_cte as (
    select 1
),
-- Comment
other_cte as (
    select 1
)

select * from my_cte cross join other_cte";

        let fixed = fix(sql.into(), rules());

        assert_eq!(
            fixed,
            "
with my_cte as (
    select 1
),

-- Comment
other_cte as (
    select 1
)

select * from my_cte cross join other_cte"
        );
    }

    #[test]
    fn test_fail_no_blank_line_after_cte_and_comment() {
        let sql = "
WITH mycte AS (
  SELECT col
  FROM
    my_table
)  /* cte comment */
SELECT col
FROM
  mycte";
        let fixed = fix(sql.into(), rules());

        assert_eq!(
            fixed,
            "
WITH mycte AS (
  SELECT col
  FROM
    my_table
)  /* cte comment */

SELECT col
FROM
  mycte"
        );
    }

    #[test]
    fn test_fail_no_blank_line_after_last_cte_trailing_comma() {
        let sql = "
with my_cte as (
    select 1
),

other_cte as (
    select 1
)
select * from my_cte cross join other_cte";
        let fixed = fix(sql.into(), rules());
        assert_eq!(
            fixed,
            "
with my_cte as (
    select 1
),

other_cte as (
    select 1
)

select * from my_cte cross join other_cte"
        );
    }

    #[test]
    fn test_fail_no_blank_line_after_last_cte_leading_comma() {
        let fail_str = "
with my_cte as (
    select 1
)

, other_cte as (
    select 1
)
select * from my_cte cross join other_cte";
        let fixed = fix(fail_str.into(), rules());

        assert_eq!(
            fixed,
            "
with my_cte as (
    select 1
)

, other_cte as (
    select 1
)

select * from my_cte cross join other_cte"
        );
    }

    #[test]
    fn test_fail_oneline_cte_leading_comma() {
        let fail_str = "
with my_cte as (select 1), other_cte as (select 1) select * from my_cte
cross join other_cte";
        let fixed = fix(fail_str.into(), rules());

        println!("{fixed}");
    }
}
