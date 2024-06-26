use std::ops::Deref;

use ahash::AHashMap;

use super::LT03::RuleLT03;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Debug, Default, Clone)]
pub struct RuleLT04 {
    base: RuleLT03,
}

impl Rule for RuleLT04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleLT04::default().erased()
    }

    fn name(&self) -> &'static str {
        "layout.commas"
    }

    fn description(&self) -> &'static str {
        "Leading/Trailing comma enforcement."
    }
    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

There is a mixture of leading and trailing commas.

```sql
SELECT
    a
    , b,
    c
FROM foo
```

**Best practice**

By default, sqruff prefers trailing commas. However it is configurable for leading commas. The chosen style must be used consistently throughout your SQL.

```sql
SELECT
    a,
    b,
    c
FROM foo

-- Alternatively, set the configuration file to 'leading'
-- and then the following would be acceptable:

SELECT
    a
    , b
    , c
FROM foo
```
"#
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if self.check_trail_lead_shortcut(
            &context.segment,
            context.parent_stack.last().unwrap(),
            "trailing",
        ) {
            return Vec::new();
        };

        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone(),
            "both",
            context.config.unwrap(),
        )
        .rebreak()
        .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["comma"].into()).into()
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

        let fix_str = fix(fail_str, rules());

        println!("{fix_str}");
    }
}
