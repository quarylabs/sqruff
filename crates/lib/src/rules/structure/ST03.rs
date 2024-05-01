use std::cell::RefCell;

use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::IndexMap;
use crate::utils::analysis::query::Query;

#[derive(Debug, Default, Clone)]
pub struct RuleST03 {}

impl Rule for RuleST03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleST03::default().erased()
    }

    fn name(&self) -> &'static str {
        "structure.unused_cte"
    }

    fn description(&self) -> &'static str {
        "Query defines a CTE (common-table expression) but does not use it."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut result = Vec::new();
        let query: Query<'_, ()> = Query::from_root(context.segment.clone(), context.dialect);

        let mut remaining_ctes: IndexMap<_, _> = RefCell::borrow(&query.inner)
            .ctes
            .keys()
            .map(|it| (it.to_uppercase(), it.clone()))
            .collect();

        for reference in context.segment.recursive_crawl(
            &["table_reference"],
            true,
            Some("with_compound_statement"),
            true,
        ) {
            remaining_ctes.shift_remove(&reference.get_raw_upper().unwrap());
        }

        for name in remaining_ctes.values() {
            let tmp = RefCell::borrow(&query.inner);
            let cte = RefCell::borrow(&tmp.ctes[name].inner);
            result.push(LintResult::new(
                cte.cte_name_segment.clone(),
                Vec::new(),
                None,
                Some(format!(
                    "Query defines CTE \"{}\" but does not use it.",
                    cte.cte_name_segment.as_ref().unwrap().get_raw().unwrap()
                )),
                None,
            ));
        }

        result
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["with_compound_statement"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::structure::ST03::RuleST03;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleST03::default().erased()]
    }

    #[test]
    fn test_pass_no_cte_defined_1() {
        let violations =
            lint("select * from t".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_1() {
        let pass_str = r#"
        with cte as (
            select
                a, b
            from 
                t
        )
        select * from cte"#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_2() {
        let pass_str = r#"
    WITH cte1 AS (
        SELECT a
        FROM t
    ),
    cte2 AS (
        SELECT b
        FROM u
    )
    SELECT *
    FROM cte1
    JOIN cte2
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_case_insensitive() {
        let pass_str = r#"
    WITH cte1 AS (
        SELECT a
        FROM t
    ),
    cte2 AS (
        SELECT b
        FROM u
    )
    SELECT *
    FROM cte1
    JOIN Cte2
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_cte_defined_but_unused_1() {
        let fail_str = r#"
    WITH cte1 AS (
        SELECT a
        FROM t
    ),
    cte2 AS (
        SELECT b
        FROM u
    )
    SELECT *
    FROM cte1
    "#;

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations[0].desc(), r#"Query defines CTE "cte2" but does not use it."#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_cte_defined_but_unused_2() {
        let fail_str = r#"
    WITH cte_orders AS (
        SELECT customer_id, total
        FROM orders
    )
    SELECT *
    FROM
        orders AS cte_orders
    "#;

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations[0].desc(), r#"Query defines CTE "cte_orders" but does not use it."#);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_cte_defined_and_used_3() {
        let pass_str = r#"
    WITH cte1 AS (
        SELECT a
        FROM t
    ),
    cte2 AS (
        SELECT b
        FROM cte1
    )
    SELECT *
    FROM cte2
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_no_cte_defined_2() {
        let pass_str = "CREATE TABLE my_table (id INTEGER)";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_4() {
        let pass_str = r#"
    WITH max_date_cte AS (
        SELECT MAX(row_updated_date) AS max_date
        FROM warehouse.loaded_monthly
    )
    SELECT stuff
    FROM warehouse.updated_weekly
    WHERE row_updated_date <= (SELECT max_date FROM max_date_cte)
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_5() {
        let pass_str = r#"
    WITH max_date_cte AS (
      SELECT MAX(row_updated_date) AS max_date
      FROM warehouse.loaded_monthly
    ),
    uses_max_date_cte AS (
      SELECT stuff
          FROM warehouse.updated_weekly
          WHERE row_updated_date <= (SELECT max_date FROM max_date_cte)
    )
    SELECT stuff
    FROM uses_max_date_cte
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_cte_defined_and_used_6() {
        let pass_str = r#"
    with pages_xf as (
      select pages.received_at
      from pages
      where pages.received_at > (select max(received_at) from pages_xf)
    ),
    final as (
      select pages_xf.received_at
      from pages_xf
    )
    select * from final
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_cte_defined_but_unused_4() {
        let fail_str = r#"
    with pages_xf as (
      select pages.received_at
      from pages
      where pages.received_at > (select max(received_at) from pages_xf)
    ),
    final as (
      select pages_xf.received_at
      from pages_xf
    ),
    unused as (
      select pages.received_at from pages
    )
    select * from final
    "#;

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert!(!violations.is_empty(), "Expected violations for unused CTE, but none were found.");
    }

    #[test]
    fn test_pass_cte_defined_and_used_7() {
        let pass_str = r#"
    with pages_xf as (
      select pages.received_at
      from pages
      where pages.received_at > (select max(received_at) from final)
    ),
    final as (
      select pages_xf.received_at
      from pages_xf
    )
    select * from final
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore = "snowflake"]
    fn test_snowflake_delete_cte() {
        let fail_str = r#"
    DELETE FROM MYTABLE1
    USING (
        WITH MYCTE AS (SELECT COLUMN2 FROM MYTABLE3)
        SELECT COLUMN3 FROM MYTABLE3
    ) X
    WHERE COLUMN1 = X.COLUMN3
    "#;

        // Assume `lint` can be configured for different SQL dialects, here using
        // 'snowflake' as a parameter
        let violations = lint(fail_str.into(), "snowflake".into(), rules(), None, None).unwrap();
        assert!(
            !violations.is_empty(),
            "Expected violations for incorrect CTE usage in DELETE operation, but none were found."
        );
    }

    #[test]
    fn test_pass_nested_query() {
        let pass_str = r#"
    WITH
    foo AS (
        SELECT
            *
        FROM
            zipcode
    ),

    bar AS (
        SELECT
            *
        FROM
            county
    ),

    stage AS (
        (SELECT
            *
        FROM
            foo)

            UNION ALL

        (SELECT
            *
        FROM
            bar)
    )

    SELECT
        *
    FROM
        stage
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_nested_query() {
        let fail_str = r#"
    WITH
    foo AS (
        SELECT
            *
        FROM
            zipcode
    ),

    bar AS (
        SELECT
            *
        FROM
            county
    ),

    stage AS (
        (SELECT
            *
        FROM
            foo)

            UNION ALL

        (SELECT
            *
        FROM
            foo)
    )

    SELECT
        *
    FROM
        stage
    "#;

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations[0].desc(), "Query defines CTE \"bar\" but does not use it.");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_nested_query_in_from_clause() {
        let pass_str = r#"
    WITH
    foo AS (
        SELECT
            *
        FROM
            zipcode
    ),

    stage AS (
        SELECT
            *
        FROM
            (
                SELECT * FROM foo
            )
    )

    SELECT
        *
    FROM
        stage
    "#;

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_nested_query_in_from_clause() {
        let fail_str = r#"
    WITH
    foo AS (
        SELECT
            *
        FROM
            zipcode
    ),

    stage AS (
        SELECT
            *
        FROM
            (
                SELECT * FROM foofoo
            )
    )

    SELECT
        *
    FROM
        stage
    "#;

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations[0].desc(), "Query defines CTE \"foo\" but does not use it.");
        assert_eq!(violations.len(), 1);
    }
}
