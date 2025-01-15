use ahash::AHashMap;
use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

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
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP03 {
            base: RuleCP01 {
                capitalisation_policy: config["extended_capitalisation_policy"]
                    .as_string()
                    .unwrap()
                    .into(),
                description_elem: "Function names",
                ignore_words: config["ignore_words"]
                    .map(|it| {
                        it.as_array()
                            .unwrap()
                            .iter()
                            .map(|it| it.as_string().unwrap().to_lowercase())
                            .collect()
                    })
                    .unwrap_or_default(),
                ignore_words_regex: config["ignore_words_regex"]
                    .map(|it| {
                        it.as_array()
                            .unwrap()
                            .iter()
                            .map(|it| Regex::new(it.as_string().unwrap()).unwrap())
                            .collect()
                    })
                    .unwrap_or_default(),

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
        SegmentSeekerCrawler::new(const {SyntaxSet::new(&[
            SyntaxKind::FunctionNameIdentifier,
            SyntaxKind::BareFunction,
        ]) })
        .into()
    }
}
