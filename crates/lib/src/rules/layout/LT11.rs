use std::collections::HashSet;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT11;

impl Rule for RuleLT11 {
    fn name(&self) -> &'static str {
        "layout.set_operators"
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["set_operator"])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone_box(),
            "both",
        )
        .rebreak()
        .results()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT11;
    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT11::default().erased()]
    }

    #[test]
    #[ignore]
    fn test_fail_simple_fix_union_all_before() {
        let sql = r#"SELECT a UNION ALL SELECT b"#;

        let result = fix(sql.into(), rules());
        println!("{}", result);
    }
}
