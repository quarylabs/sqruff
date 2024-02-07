use std::collections::HashSet;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::Boxed;

#[derive(Debug, Default)]
pub struct RuleLT04 {}

impl Rule for RuleLT04 {
    fn crawl_behaviour(&self) -> Box<dyn Crawler> {
        SegmentSeekerCrawler::new(HashSet::from(["comma".into()])).boxed()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        todo!()
    }
}
