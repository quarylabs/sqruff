use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::{SymbolSegment, SymbolSegmentNewArgs};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
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
pub struct RuleCv02 {}

impl Rule for RuleCv02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleCv02::default().erased()
    }

    fn name(&self) -> &'static str {
        "convention.coalesce"
    }

    fn description(&self) -> &'static str {
        "Use 'COALESCE' instead of 'IFNULL' or 'NVL'."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        // Use "COALESCE" instead of "IFNULL" or "NVL".
        // We only care about function names, and they should be the
        // only things we get.
        // assert!(context.segment.is_type("function_name_identifier"));

        // Only care if the function is "IFNULL" or "NVL".

        if !["IFNULL", "NVL"].contains(&context.segment.get_raw_upper().unwrap().as_str()) {
            return Vec::new();
        }

        // Create fix to replace "IFNULL" or "NVL" with "COALESCE".
        let fix = LintFix::replace(
            context.segment.clone(),
            vec![SymbolSegment::create(
                "COALESCE",
                &<_>::default(),
                SymbolSegmentNewArgs { r#type: "function_name_identifier" },
            )],
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
        SegmentSeekerCrawler::new(["function_name_identifier"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::dialects::init::get_default_dialect;
    use crate::core::rules::base::Erased;
    use crate::rules::convention::CV02::RuleCv02;

    #[test]
    fn test__rules__std_CV02_raised() {
        // CV02 is raised for use of "IFNULL" or "NVL".
        let sql = "SELECT\n\tIFNULL(NULL, 100),\n\tNVL(NULL, 100);";
        let result = lint(
            sql.to_string(),
            get_default_dialect().to_string(),
            vec![RuleCv02::default().erased()],
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
            sql.to_string(),
            get_default_dialect().to_string(),
            vec![RuleCv02::default().erased()],
            None,
            None,
        )
        .unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn test_fail_ifnull() {
        let sql = "SELECT ifnull(foo, 0) AS bar,\nFROM baz;";
        let result = fix(sql.to_string(), vec![RuleCv02::default().erased()]);
        assert_eq!(result, "SELECT COALESCE(foo, 0) AS bar,\nFROM baz;")
    }

    #[test]
    fn test_fail_nvl() {
        let sql = "SELECT nvl(foo, 0) AS bar,\nFROM baz;";
        let result = fix(sql.to_string(), vec![RuleCv02::default().erased()]);
        assert_eq!(result, "SELECT COALESCE(foo, 0) AS bar,\nFROM baz;")
    }
}
