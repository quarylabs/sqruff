use ahash::AHashSet;

use crate::core::parser::segments::base::{NewlineSegment, Segment};
use crate::core::rules::base::{LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT07 {}

impl Rule for RuleLT07 {
    fn name(&self) -> &'static str {
        "layout.cte_bracket"
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["with_compound_statement"].into()).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let segments = FunctionalContext::new(context.clone())
            .segment()
            .children(Some(|seg| seg.is_type("common_table_expression")));

        let mut cte_end_brackets = AHashSet::new();
        for cte in segments.iterate_segments() {
            let cte_start_bracket = cte
                .children(None)
                .find_last(Some(|seg| seg.is_type("bracketed")))
                .children(None)
                .find_first(Some(|seg: &dyn Segment| seg.is_type("start_bracket")));

            let cte_end_bracket = cte
                .children(None)
                .find_last(Some(|seg| seg.is_type("bracketed")))
                .children(None)
                .find_first(Some(|seg: &dyn Segment| seg.is_type("end_bracket")));

            if !cte_start_bracket.is_empty() && !cte_end_bracket.is_empty() {
                if cte_start_bracket[0].get_position_marker().unwrap().line_no()
                    == cte_end_bracket[0].get_position_marker().unwrap().line_no()
                {
                    continue;
                }
                cte_end_brackets.insert(cte_end_bracket[0].clone_box());
            }
        }

        for seg in cte_end_brackets {
            let mut contains_non_whitespace = false;
            let idx = context.segment.get_raw_segments().iter().position(|it| it == &seg).unwrap();
            if idx > 0 {
                for elem in context.segment.get_raw_segments()[..idx].iter().rev() {
                    if elem.is_type("newline") {
                        break;
                    } else if !(elem.is_type("indent") || elem.is_type("whitespace")) {
                        contains_non_whitespace = true;
                        break;
                    }
                }
            }

            if contains_non_whitespace {
                return vec![LintResult::new(
                    seg.clone().into(),
                    vec![LintFix::create_before(
                        seg,
                        vec![NewlineSegment::new("\n", &<_>::default(), <_>::default())],
                    )],
                    None,
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT07::RuleLT07;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT07::default().erased()]
    }

    #[test]
    fn test_pass_with_clause_closing_aligned() {
        let pass_str = "
with cte as (
    select 1
) select * from cte";

        let result = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(result, []);
    }

    #[test]
    fn test_pass_with_clause_closing_oneline() {
        let pass_str = "with cte as (select 1) select * from cte";

        let result = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(result, []);
    }

    #[test]
    fn test_pass_with_clause_closing_misaligned_indentation() {
        let pass_str = "
with cte as (
    select 1
    ) select * from cte";

        let result = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(result, []);
    }

    #[test]
    fn test_pass_with_clause_closing_misaligned_negative_indentation() {
        let pass_str = "
with cte as (
    select 1
    ) select * from cte";

        let result = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(result, []);
    }

    #[test]
    fn test_move_parenthesis_to_next_line() {
        let sql = "
with cte_1 as (
    select foo
    from tbl_1) -- Foobar
    
select cte_1.foo
from cte_1";

        let fixed = fix(sql.into(), rules());
        assert_eq!(
            fixed,
            "
with cte_1 as (
    select foo
    from tbl_1
) -- Foobar
    
select cte_1.foo
from cte_1"
        );
    }

    #[test]
    fn test_pass_cte_with_column_list() {
        let violations = lint(
            "
with
search_path (node_ids, total_time) as (
    select 1
)
select * from search_path"
                .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }
}
