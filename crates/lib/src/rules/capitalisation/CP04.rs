use ahash::AHashMap;

use super::CP01::RuleCP01;
use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};

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
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCP04 {
            base: RuleCP01 {
                capitalisation_policy: _config["capitalisation_policy"].as_string().unwrap().into(),
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
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Capitalisation]
    }
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleCP04;
    use crate::api::simple::fix;
    use crate::core::rules::base::Erased;

    #[test]
    fn test_fail_inconsistent_boolean_capitalisation() {
        let fail_str = "SeLeCt true, FALSE, NULL";
        let fix_str = "SeLeCt true, false, null";

        let actual = fix(fail_str, vec![RuleCP04::default().erased()]);
        assert_eq!(fix_str, actual);
    }
}
