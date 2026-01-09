use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::RuleCP01;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};

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
        let rules = &context.config.rules.capitalisation_literals;
        self.base.eval_with_config(
            context,
            &rules.capitalisation_policy,
            &rules.ignore_words,
            &rules.ignore_words_regex,
        )
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
