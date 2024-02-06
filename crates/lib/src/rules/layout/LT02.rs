use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::helpers::Boxed;
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Default, Debug)]
pub struct RuleLT02 {}

impl Rule for RuleLT02 {
    fn crawl_behaviour(&self) -> Box<dyn Crawler> {
        RootOnlyCrawler::default().boxed()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_root(context.segment, context.config.clone()).reindent().results()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::errors::SQLLintError;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT02::RuleLT02;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT02::default().erased()]
    }

    #[test]
    #[ignore]
    fn test_fail_reindent_first_line_1() {
        let fail_str = "     SELECT 1";
        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(
            violations,
            [SQLLintError { description: "First line should not be indented.".into() }]
        );
    }
}
