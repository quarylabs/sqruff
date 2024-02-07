use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::core::parser::segments::base::NewlineSegment;
use crate::core::rules::base::{EditType, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::Boxed;

#[derive(Debug, Default)]
pub struct RuleLT08 {}

impl Rule for RuleLT08 {
    fn crawl_behaviour(&self) -> Box<dyn Crawler> {
        SegmentSeekerCrawler::new(HashSet::from(["with_compound_statement"])).boxed()
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
            let mut seg_idx: usize = 1;
            let mut line_idx: usize = 0;
            let mut comma_seg_idx: usize = 0;
            let mut blank_lines: usize = 0;
            let mut comma_line_idx: Option<usize> = None;
            let mut line_blank: bool = false;
            let comma_style: String;
            let mut line_starts: HashMap<usize, usize> = HashMap::new();
            let mut comment_lines: Vec<usize> = Vec::new();

            while forward_slice[seg_idx].is_type("comma") || !forward_slice[seg_idx].is_code() {
                if forward_slice[seg_idx].is_type("newline") {
                    if line_blank {
                        blank_lines += 1;
                    }

                    line_blank = true;
                    line_idx += 1;
                    line_starts.insert(line_idx, seg_idx + 1);
                } else if forward_slice[seg_idx].is_type("comment") {
                    line_blank = false;
                    comment_lines.push(line_idx);
                }
                seg_idx += 1;
            }

            let comma_style = match comma_line_idx {
                None => "final",
                Some(idx) if idx == 0 => "oneline",
                Some(0) => "trailing",
                Some(idx) if idx == line_idx => "leading",
                _ => "floating",
            };

            if blank_lines >= 1 {
                continue;
            }

            let mut fix_type = EditType::CreateBefore;
            let mut fix_point = None;

            if comma_style == "oneline" {
                unimplemented!()
            } else {
                if comment_lines.is_empty() || comment_lines.contains(&(line_idx - 1)) {
                    if matches!(comma_style, "trailing" | "final" | "floating") {
                        if forward_slice[seg_idx - 1].is_type("whitespace") {
                            fix_point = forward_slice[seg_idx - 1].clone().into();
                            fix_type = EditType::Replace;
                        } else {
                            fix_point = forward_slice[seg_idx].clone().into();
                        }
                    } else {
                    }
                } else if comma_style == "leading" {
                    fix_point = forward_slice[comma_seg_idx].clone().into();
                } else {
                    unimplemented!()
                }
            }

            let num_newlines = 1;

            let fixes = vec![LintFix {
                edit_type: fix_type,
                anchor: fix_point.unwrap(),
                edit: Some(NewlineSegment::new("\n", &<_>::default(), <_>::default()))
                    .into_iter()
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

        dbg!(&error_buffer);

        error_buffer
    }
}

#[cfg(test)]
mod tests {
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

        let lints = fix(sql.into(), rules());
        println!("{lints}");
    }
}
