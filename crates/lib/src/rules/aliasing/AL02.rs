use ahash::AHashMap;

use super::AL01::{Aliasing, RuleAL01};
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::utils::functional::context::FunctionalContext;

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

impl RuleAL02 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.base = self.base.aliasing(aliasing);
        self
    }
}

impl Rule for RuleAL02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL02::default().erased())
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

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        if FunctionalContext::new(context.clone()).segment().children(None).last().unwrap().raw()
            == "="
        {
            return Vec::new();
        }

        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::Erased;
    use crate::rules::aliasing::AL01::Aliasing;
    use crate::rules::aliasing::AL02::RuleAL02;

    #[test]
    fn issue_561() {
        let pass_str: String = "select
        array_agg(catalog_item_id) within group
          (order by product_position asc) over (partition by (event_id, shelf_position))
        as shelf_catalog_items
      from x"
            .into();

        let violations =
            lint(pass_str, "snowflake".into(), vec![RuleAL02::default().erased()], None, None)
                .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_explicit_column_default() {
        assert_eq!(
            fix("select 1 bar from table1 b", vec![RuleAL02::default().erased()]),
            "select 1 AS bar from table1 b"
        );
    }

    #[test]
    fn test_fail_explicit_column_explicit() {
        let sql = "select 1 bar from table1 b";

        let result = fix(sql, vec![RuleAL02::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select 1 AS bar from table1 b");
    }

    #[test]
    fn test_fail_explicit_column_implicit() {
        let sql = "select 1 AS bar from table1 b";

        let result = fix(sql, vec![RuleAL02::default().aliasing(Aliasing::Implicit).erased()]);

        assert_eq!(result, "select 1 bar from table1 b");
    }

    #[test]
    fn test_fail_alias_ending_raw_equals() {
        let sql = "select col1 raw_equals";
        let result = fix(sql, vec![RuleAL02::default().aliasing(Aliasing::Explicit).erased()]);

        assert_eq!(result, "select col1 AS raw_equals");
    }
}
