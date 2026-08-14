use hashbrown::HashMap;
use regex::Regex;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::{CapitalisationPolicy, RuleCP01};
use crate::core::config::Value;
use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug)]
pub struct RuleCP04 {
    base: RuleCP01,
}

impl Default for RuleCP04 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                skip_literals: false,
                exclude_parent_types: &[],
                description_elem: "Boolean/null literals",
                ..Default::default()
            },
        }
    }
}

crate::rule_config! {
    /// Configuration for `capitalisation.literals` (CP04).
    RuleCP04Config {
        /// The capitalisation to enforce on `NULL` and boolean literals.
        capitalisation_policy: CapitalisationPolicy = CapitalisationPolicy::Consistent,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

impl Rule for RuleCP04 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleCP04Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleCP04Config::from_config(config)?;

        Ok(RuleCP04 {
            base: RuleCP01 {
                capitalisation_policy: config.capitalisation_policy.into(),
                ignore_words: config.ignore_words,
                ignore_words_regex: config.ignore_words_regex,
                ..Default::default()
            },
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.literals"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of boolean/null literal."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `null` and `false` are in lower-case whereas `TRUE` is in upper-case.

```sql
select
    a,
    null,
    TRUE,
    false
from foo
```

**Best practice**

Ensure all literal `null`/`true`/`false` literals are consistently upper or lower case

```sql
select
    a,
    NULL,
    TRUE,
    FALSE
from foo

-- Also good

select
    a,
    null,
    true,
    false
from foo
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
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::NullLiteral, SyntaxKind::BooleanLiteral]) },
        )
        .into()
    }
}
