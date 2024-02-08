use std::collections::HashSet;
use std::ops::Deref;

use super::LT03::RuleLT03;
use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default)]
pub struct RuleLT04 {
    base: RuleLT03,
}

impl Rule for RuleLT04 {
    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["comma".into()])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if self.check_trail_lead_shortcut(
            context.segment.as_ref(),
            context.parent_stack.last().unwrap().as_ref(),
            "trailing",
        ) {
            return Vec::new();
        };

        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone(),
            "both",
        )
        .rebreak()
        .results()
    }
}

impl Deref for RuleLT04 {
    type Target = RuleLT03;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

#[cfg(test)]
mod tests {

    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT04::RuleLT04;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT04::default().erased()]
    }

    #[test]
    fn leading_comma_violations() {
        let fail_str = "
SELECT
  a
  , b
FROM c";

        let fix_str = fix(fail_str.into(), rules());

        println!("{fix_str}");
    }
}
