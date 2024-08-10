use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::SegmentBuilder;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};

/// Prefer using `COALESCE` over `IFNULL` or `NVL`.
///
/// # Anti-pattern
///
/// `IFNULL` or `NVL` are commonly used to handle `NULL` values in SQL queries.
/// However, they have compatibility issues across different database systems.
///
/// ```sql
/// SELECT ifnull(foo, 0) AS bar,
/// FROM baz;
///
/// SELECT nvl(foo, 0) AS bar,
/// FROM baz;
/// ```
///
/// # Best Practice
///
/// It is recommended to use `COALESCE` instead. `COALESCE` is universally
/// supported, while `IFNULL` is not supported in Redshift, and `NVL` is not
/// supported in BigQuery. Moreover, `COALESCE` offers greater flexibility, as
/// it can accept an arbitrary number of arguments, enhancing the query's
/// robustness.
///
/// ```sql
/// SELECT coalesce(foo, 0) AS bar,
/// FROM baz;
/// ```
#[derive(Debug, Default, Clone)]
pub struct RuleCV02;

impl Rule for RuleCV02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV02.erased())
    }

    fn name(&self) -> &'static str {
        "convention.coalesce"
    }

    fn description(&self) -> &'static str {
        "Use 'COALESCE' instead of 'IFNULL' or 'NVL'."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

`IFNULL` or `NVL` are used to fill `NULL` values.

```sql
SELECT ifnull(foo, 0) AS bar,
FROM baz;

SELECT nvl(foo, 0) AS bar,
FROM baz;
```

**Best practice**

Use COALESCE instead. COALESCE is universally supported, whereas Redshift doesn’t support IFNULL and BigQuery doesn’t support NVL. Additionally, COALESCE is more flexible and accepts an arbitrary number of arguments.

```sql
SELECT coalesce(foo, 0) AS bar,
FROM baz;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        // Use "COALESCE" instead of "IFNULL" or "NVL".
        // We only care about function names, and they should be the
        // only things we get.
        // assert!(context.segment.is_type(SyntaxKind::FunctionNameIdentifier));

        // Only care if the function is "IFNULL" or "NVL".

        if !["IFNULL", "NVL"].contains(&context.segment.get_raw_upper().unwrap().as_str()) {
            return Vec::new();
        }

        // Create fix to replace "IFNULL" or "NVL" with "COALESCE".
        let fix = LintFix::replace(
            context.segment.clone(),
            vec![
                SegmentBuilder::token(
                    context.tables.next_id(),
                    "COALESCE",
                    SyntaxKind::FunctionNameIdentifier,
                )
                .finish(),
            ],
            None,
        );

        vec![LintResult::new(
            context.segment.clone().into(),
            vec![fix],
            None,
            Some(format!(
                "Use 'COALESCE' instead of '{}'.",
                context.segment.get_raw_upper().unwrap()
            )),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::FunctionNameIdentifier]) })
            .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::dialects::init::get_default_dialect;
    use crate::core::rules::base::Erased;
    use crate::rules::convention::cv02::RuleCV02;

    #[test]
    fn test_rules_std_cv02_raised() {
        // CV02 is raised for use of "IFNULL" or "NVL".
        let sql = "SELECT\n\tIFNULL(NULL, 100),\n\tNVL(NULL, 100);";
        let result = lint(
            sql.into(),
            get_default_dialect().to_string(),
            vec![RuleCV02.erased()],
            None,
            None,
        )
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].description, "Use 'COALESCE' instead of 'IFNULL'.");
        assert_eq!(result[1].description, "Use 'COALESCE' instead of 'NVL'.");
    }

    #[test]
    fn test_pass_coalesce() {
        let sql = "SELECT coalesce(foo, 0) AS bar,\nFROM baz;";

        let result = lint(
            sql.into(),
            get_default_dialect().to_string(),
            vec![RuleCV02.erased()],
            None,
            None,
        )
        .unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn test_fail_ifnull() {
        let sql = "SELECT ifnull(foo, 0) AS bar,\nFROM baz;";
        let result = fix(sql, vec![RuleCV02.erased()]);
        assert_eq!(result, "SELECT COALESCE(foo, 0) AS bar,\nFROM baz;")
    }

    #[test]
    fn test_fail_nvl() {
        let sql = "SELECT nvl(foo, 0) AS bar,\nFROM baz;";
        let result = fix(sql, vec![RuleCV02.erased()]);
        assert_eq!(result, "SELECT COALESCE(foo, 0) AS bar,\nFROM baz;")
    }
}
