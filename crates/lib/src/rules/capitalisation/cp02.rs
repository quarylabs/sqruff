use hashbrown::HashMap;
use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::{CapitalisationPolicyName, ExtendedCapitalisationPolicy, RuleCP01};
use crate::core::config::Value;
use crate::core::rules::config::{IgnoreWords, RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::{IdentifiersPolicy, identifiers_policy_applicable};

crate::rule_config! {
    /// Configuration for `capitalisation.identifiers` (CP02).
    RuleCP02Config {
        /// The capitalisation to enforce on unquoted identifiers.
        extended_capitalisation_policy: ExtendedCapitalisationPolicy =
            ExtendedCapitalisationPolicy::Consistent,
        /// Which unquoted identifiers the rule applies to.
        unquoted_identifiers_policy: IdentifiersPolicy = IdentifiersPolicy::All,
        /// Comma separated list of words to ignore, compared case-insensitively.
        ignore_words: IgnoreWords = IgnoreWords::default(),
        /// Comma separated list of regular expressions matching words to ignore.
        ignore_words_regex: Vec<Regex> = Vec::new(),
    }
}

#[derive(Clone, Debug)]
pub struct RuleCP02 {
    base: RuleCP01,
    unquoted_identifiers_policy: IdentifiersPolicy,
}

impl Default for RuleCP02 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                cap_policy_name: CapitalisationPolicyName::Extended,
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
            unquoted_identifiers_policy: IdentifiersPolicy::All,
        }
    }
}

impl Rule for RuleCP02 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleCP02Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleCP02Config::from_config(config)?;

        Ok(RuleCP02 {
            base: RuleCP01 {
                capitalisation_policy: config.extended_capitalisation_policy,
                cap_policy_name: CapitalisationPolicyName::Extended,
                description_elem: "Unquoted identifiers",
                ignore_words: config.ignore_words,
                ignore_words_regex: config.ignore_words_regex,
                ..Default::default()
            },
            unquoted_identifiers_policy: config.unquoted_identifiers_policy,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "capitalisation.identifiers"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of unquoted identifiers."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, unquoted identifier `a` is in lower-case but `B` is in upper-case.

```sql
select
    a,
    B
from foo
```

**Best practice**

Ensure all unquoted identifiers are either in upper-case or in lower-case.

```sql
select
    a,
    b
from foo

-- Also good

select
    A,
    B
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
        // TODO: add databricks
        if context.dialect.name == DialectKind::Sparksql
            && context
                .parent_stack
                .last()
                .is_some_and(|it| it.get_type() == SyntaxKind::PropertyNameIdentifier)
            && context.segment.raw() == "enableChangeDataFeed"
        {
            return Vec::new();
        }

        if identifiers_policy_applicable(self.unquoted_identifiers_policy, &context.parent_stack) {
            self.base.eval(context)
        } else {
            vec![LintResult::new(None, Vec::new(), None, None)]
        }
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::NakedIdentifier,
                    SyntaxKind::PropertiesNakedIdentifier,
                ])
            },
        )
        .into()
    }
}
