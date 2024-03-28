use std::collections::HashMap;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::analysis::query::Query;

#[derive(Debug, Default, Clone)]
pub struct RuleST03 {}

impl Rule for RuleST03 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["with_compound_statement"].into()).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let result = Vec::new();
        let query: Query<'_, ()> = Query::from_root(context.segment.clone(), &context.dialect);

        let remaining_ctes: HashMap<_, _> =
            query.ctes.keys().map(|it| (it.to_uppercase(), it.clone())).collect();

        dbg!(remaining_ctes);

        result
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::structure::ST03::RuleST03;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleST03::default().erased()]
    }

    #[test]
    fn test_pass_no_cte_defined_1() {
        let violations =
            lint("select * from t".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_1() {
        let pass_str = r#"
        with cte as (
            select
                a, b
            from 
                t
        )
        select * from cte"#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }
}
