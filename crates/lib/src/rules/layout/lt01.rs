use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::{Filter, ReflowSequence};

#[derive(Default, Debug, Clone)]
pub struct RuleLT01;

impl Rule for RuleLT01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT01.erased())
    }
    fn name(&self) -> &'static str {
        "layout.spacing"
    }

    fn description(&self) -> &'static str {
        "Inappropriate Spacing."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, spacing is all over the place and is represented by `•`.

```sql
SELECT
    a,        b(c) as d••
FROM foo••••
JOIN bar USING(a)
```

**Best practice**

- Unless an indent or preceding a comment, whitespace should be a single space.
- There should also be no trailing whitespace at the ends of lines.
- There should be a space after USING so that it’s not confused for a function.

```sql
SELECT
    a, b(c) as d
FROM foo
JOIN bar USING (a)
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let sequence = ReflowSequence::from_root(context.segment.clone(), context.config);
        sequence
            .respace(context.tables, false, Filter::All)
            .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
