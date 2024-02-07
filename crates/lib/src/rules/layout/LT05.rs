use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::helpers::Boxed;

#[derive(Debug, Default)]
pub struct RuleLT05 {}

impl Rule for RuleLT05 {
    fn crawl_behaviour(&self) -> Box<dyn Crawler> {
        RootOnlyCrawler::default().boxed()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        todo!()
    }
}
