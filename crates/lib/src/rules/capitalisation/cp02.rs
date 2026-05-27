use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::config::{IdentifierPolicy, RuleConfigs};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeeker};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Clone, Debug)]
pub struct RuleCP02 {
    base: RuleCP01,
    unquoted_identifiers_policy: IdentifierPolicy,
}

impl Default for RuleCP02 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
            unquoted_identifiers_policy: IdentifierPolicy::All,
        }
    }
}

impl Rule for RuleCP02 {
    fn load_from_config(&self, config: &RuleConfigs) -> Result<ErasedRule, String> {
        Ok(RuleCP02 {
            base: RuleCP01 {
                capitalisation_policy: config
                    .capitalisation
                    .identifiers
                    .extended_capitalisation_policy
                    .as_str()
                    .into(),
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
                ignore_words: config.capitalisation.identifiers.ignore_words.clone(),
                ignore_words_regex: config.capitalisation.identifiers.ignore_words_regex.clone(),

                ..Default::default()
            },
            unquoted_identifiers_policy: config
                .capitalisation
                .identifiers
                .unquoted_identifiers_policy,
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

        if identifiers_policy_applicable(
            self.unquoted_identifiers_policy.as_str(),
            &context.parent_stack,
        ) {
            self.base.eval(context)
        } else {
            vec![LintResult::new(None, Vec::new(), None, None)]
        }
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeeker::new(
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
