use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::config::RuleConfigs;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeeker};
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

impl Rule for RuleCP04 {
    fn load_from_config(&self, config: &RuleConfigs) -> Result<ErasedRule, String> {
        Ok(RuleCP04 {
            base: RuleCP01 {
                capitalisation_policy: config
                    .capitalisation
                    .literals
                    .capitalisation_policy
                    .as_str()
                    .into(),
                ignore_words: config.capitalisation.literals.ignore_words.clone(),
                ignore_words_regex: config.capitalisation.literals.ignore_words_regex.clone(),
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
        SegmentSeeker::new(
            const { SyntaxSet::new(&[SyntaxKind::NullLiteral, SyntaxKind::BooleanLiteral]) },
        )
        .into()
    }
}
