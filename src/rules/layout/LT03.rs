use std::collections::HashSet;

use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::Boxed;

#[derive(Debug, Default)]
pub struct RuleLT03 {}

impl Rule for RuleLT03 {
    fn crawl_behaviour(&self) -> Box<dyn Crawler> {
        SegmentSeekerCrawler::new(HashSet::from(["binary_operator", "comparison_operator"])).boxed()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleLT03;
    use crate::api::simple::lint;
    use crate::core::rules::base::Erased;

    #[test]
    fn passes_on_before_default() {
        let sql = r#"
select
    a
    + b
from foo
"#;

        let result =
            lint(sql.into(), "ansi".into(), vec![RuleLT03::default().erased()], None, None)
                .unwrap();

        assert_eq!(result, &[]);
    }
}
