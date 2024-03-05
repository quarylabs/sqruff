use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT05 {
    ignore_comment_lines: bool,
    ignore_comment_clauses: bool,
}

impl Rule for RuleLT05 {
    fn name(&self) -> &'static str {
        "layout.long_lines"
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler::default().into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let results =
            ReflowSequence::from_root(context.segment, context.config).break_long_lines().results();

        if self.ignore_comment_lines {
            unimplemented!()
        }

        if self.ignore_comment_clauses {
            unimplemented!()
        }

        results
    }
}
