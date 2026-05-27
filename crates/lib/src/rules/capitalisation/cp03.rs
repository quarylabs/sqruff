use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::config::RuleConfigs;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeeker};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleCP03 {
    base: RuleCP01,
}

impl Default for RuleCP03 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                skip_literals: false,
                exclude_parent_types: &[],
                ..Default::default()
            },
        }
    }
}

impl Rule for RuleCP03 {
    fn load_from_config(&self, config: &RuleConfigs) -> Result<ErasedRule, String> {
        Ok(RuleCP03 {
            base: RuleCP01 {
                capitalisation_policy: config
                    .capitalisation
                    .functions
                    .extended_capitalisation_policy
                    .as_str()
                    .into(),
                description_elem: "Function names",
                ignore_words: config.capitalisation.functions.ignore_words.clone(),
                ignore_words_regex: config.capitalisation.functions.ignore_words_regex.clone(),

                ..Default::default()
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.functions"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of function names."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the two `SUM` functions don’t have the same capitalisation.

```sql
SELECT
    sum(a) AS aa,
    SUM(b) AS bb
FROM foo
```

**Best practice**

Make the case consistent.


```sql
SELECT
    sum(a) AS aa,
    sum(b) AS bb
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[
            RuleGroups::All,
            RuleGroups::Core,
            RuleGroups::Capitalisation,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        self.base.eval(context)
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeeker::new(const {SyntaxSet::new(&[
            SyntaxKind::FunctionNameIdentifier,
            SyntaxKind::BareFunction,
        ]) })
        .into()
    }
}
