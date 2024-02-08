use std::collections::HashSet;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default)]
pub struct RuleLT07 {}

impl Rule for RuleLT07 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["with_compound_statement"])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
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
    #[ignore = "parser bug"]
    fn test_move_parenthesis_to_next_line() {
        let sql = "
with cte_1 as (
    select foo
    from tbl_1) -- Foobar
    
select cte_1.foo
from cte_1";

        let result = fix(sql.into(), rules());
        dbg!(result);
    }
}
