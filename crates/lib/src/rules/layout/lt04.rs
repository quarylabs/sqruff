use std::ops::Deref;

use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::lt03::RuleLT03;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

#[derive(Debug, Default, Clone)]
pub struct RuleLT04 {
    base: RuleLT03,
}

impl Rule for RuleLT04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT04::default().erased())
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
    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let comma_positioning = context.config.raw["layout"]["type"]["comma"]["line_position"]
            .as_string()
            .unwrap();

        if self.check_trail_lead_shortcut(
            &context.segment,
            context.parent_stack.last().unwrap(),
            comma_positioning,
        ) {
            return vec![LintResult::new(None, Vec::new(), None, None)];
        };

        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone(),
            TargetSide::Both,
            context.config,
        )
        .rebreak(context.tables)
        .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Comma]) }).into()
    }
}

impl Deref for RuleLT04 {
    type Target = RuleLT03;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
