use hashbrown::HashMap;
use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::{ExtendedCapitalisationPolicy, RuleCP01};
use crate::core::config::Value;
use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
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

crate::rule_config! {
    /// Configuration for `capitalisation.functions` (CP03).
    RuleCP03Config {
        /// The capitalisation to enforce on function names.
        extended_capitalisation_policy: ExtendedCapitalisationPolicy =
            ExtendedCapitalisationPolicy::Consistent,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

impl Rule for RuleCP03 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleCP03Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleCP03Config::from_config(config)?;

        Ok(RuleCP03 {
            base: RuleCP01 {
                capitalisation_policy: config.extended_capitalisation_policy,
                description_elem: "Function names",
                ignore_words: config.ignore_words,
                ignore_words_regex: config.ignore_words_regex,
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
