use super::al01::RuleAL01;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

#[derive(Debug, Clone)]
pub struct RuleAL02 {
    base: RuleAL01,
}

impl Default for RuleAL02 {
    fn default() -> Self {
        Self {
            base: RuleAL01::default()
                .target_parent_types(const { SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) }),
        }
    }
}

impl Rule for RuleAL02 {
    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "aliasing.column"
    }

    fn description(&self) -> &'static str {
        "Implicit/explicit aliasing of columns."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the alias for column `a` is implicit.

```sql
SELECT
  a alias_col
FROM foo
```

**Best practice**

Add the `AS` keyword to make the alias explicit.

```sql
SELECT
    a AS alias_col
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if matches!(
            context.segment
                .child(&SyntaxSet::new(&[SyntaxKind::AliasOperator])),
            Some(alias_operator) if alias_operator.raw() == "="
        ) {
            return Vec::new();
        }

        let aliasing = context.config.rules.aliasing_column.aliasing;
        self.base.eval_with_aliasing(context, aliasing)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
