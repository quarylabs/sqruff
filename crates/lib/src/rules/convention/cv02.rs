use ahash::AHashMap;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

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

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // Use "COALESCE" instead of "IFNULL" or "NVL".
        // We only care about function names, and they should be the
        // only things we get.
        // assert!(context.segment.is_type(SyntaxKind::FunctionNameIdentifier));

        // Only care if the function is "IFNULL" or "NVL".

        if !(context.segment.raw().eq_ignore_ascii_case("IFNULL")
            || context.segment.raw().eq_ignore_ascii_case("NVL"))
        {
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
            Some(format!(
                "Use 'COALESCE' instead of '{}'.",
                context.segment.raw().to_uppercase_smolstr()
            )),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::FunctionNameIdentifier]) })
            .into()
    }
}
