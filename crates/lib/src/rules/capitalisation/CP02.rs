use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Clone, Debug)]
pub struct RuleCP02 {}

impl Rule for RuleCP02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        todo!()
    }

    fn name(&self) -> &'static str {
        "capitalisation.identifiers"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of unquoted identifiers."
    }

    fn eval(&self, _context: RuleContext) -> Vec<LintResult> {
        todo!()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["naked_identifier", "properties_naked_identifier"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP02;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;
}
