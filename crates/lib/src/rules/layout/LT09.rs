use std::collections::HashSet;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default)]
pub struct RuleLT09 {}

impl Rule for RuleLT09 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["select_clause".into()])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        todo!()
    }
}
