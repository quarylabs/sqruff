use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT11;

impl Rule for RuleLT11 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleLT11.erased()
    }

    fn name(&self) -> &'static str {
        "layout.set_operators"
    }

    fn description(&self) -> &'static str {
        "Set operators should be surrounded by newlines."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone_box(),
            "both",
            context.config.unwrap(),
        )
        .rebreak()
        .results()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["set_operator"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT11;
    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT11.erased()]
    }

    #[test]
    fn test_fail_simple_fix_union_all_before() {
        let sql = r#"SELECT a UNION ALL SELECT b"#;

        let result = fix(sql.into(), rules());
        println!("{}", result);
    }
}
