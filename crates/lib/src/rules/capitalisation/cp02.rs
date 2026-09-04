use hashbrown::HashMap;
use regex::Regex;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::identifers::identifiers_policy_applicable;

#[derive(Clone, Debug)]
pub struct RuleCP02 {
    base: RuleCP01,
    unquoted_identifiers_policy: Option<String>,
}

impl Default for RuleCP02 {
    fn default() -> Self {
        Self {
            base: RuleCP01 {
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
                ..Default::default()
            },
            unquoted_identifiers_policy: None,
        }
    }
}

impl Rule for RuleCP02 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP02 {
            base: RuleCP01 {
                capitalisation_policy: config["extended_capitalisation_policy"]
                    .as_string()
                    .unwrap()
                    .into(),
                cap_policy_name: "extended_capitalisation_policy".into(),
                description_elem: "Unquoted identifiers",
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
            unquoted_identifiers_policy: config
                .get("unquoted_identifiers_policy")
                .and_then(|it| it.as_string())
                .map(ToString::to_string),
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
This rule applies to all unquoted identifiers, whether references or aliases, and
whether they refer to columns or other objects such as tables or schemas.

**Note:** In most dialects, unquoted identifiers are treated as case-insensitive,
so the fixes proposed by this rule do not change the interpretation of the query.
However, some databases—notably BigQuery and ClickHouse—use the casing of
unquoted identifiers when determining the casing of column headings in results.

Because this behavior is limited to a few dialects and is not widely understood,
it is considered an antipattern. If identifier case matters, quote the identifier.
If you or your organization intentionally rely on this behavior, disable this rule.

**Anti-pattern**

In this example, unquoted identifier `a` is in lower-case but `B` is in upper-case.

```sql
select
    a,
    B
from foo
```

In this more complicated example, references and aliases for columns and tables
use mixed capitalization. That inconsistency is acceptable for quoted identifiers,
but not for unquoted identifiers.

```sql
select
    col_1 + Col_2 as COL_3,
    "COL_4" as Col_5
from Foo as BAR
```

**Best practice**

Ensure all unquoted identifiers are either in upper-case or in lower-case.

```sql
select
    a,
    b
from foo;

-- ...also good...

select
    A,
    B
from foo;

-- ...or for comparison with our more complex example, this too:

select
    col_1 + col_2 as col_3,
    "COL_4" as col_5
from foo as bar
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

        let policy = self
            .unquoted_identifiers_policy
            .as_deref()
            .unwrap_or_else(|| {
                context.config.raw["rules"]["unquoted_identifiers_policy"]
                    .as_string()
                    .unwrap()
            });
        if identifiers_policy_applicable(policy, &context.parent_stack) {
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
