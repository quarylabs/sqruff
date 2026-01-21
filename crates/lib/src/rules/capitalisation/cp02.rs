use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::core::config::IdentifiersPolicy;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Clone, Debug)]
pub struct RuleCP02 {
    base: RuleCP01,
}

impl Default for RuleCP02 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                cap_policy_name: "extended_capitalisation_policy",
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
        }
    }
}

impl Rule for RuleCP02 {
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

        let rules = &context.config.rules.capitalisation_identifiers;
        let policy: IdentifiersPolicy = rules
            .unquoted_identifiers_policy
            .unwrap_or(context.config.rules.unquoted_identifiers_policy);
        if identifiers_policy_applicable(policy, &context.parent_stack) {
            self.base.eval_with_config(
                context,
                rules.extended_capitalisation_policy.as_str(),
                &rules.ignore_words,
                &rules.ignore_words_regex,
            )
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
