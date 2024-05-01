use std::cell::RefCell;

use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::ObjectReferenceLevel;
use crate::utils::analysis::query::Query;
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::segments::Segments;

#[derive(Default, Clone)]
struct AL05Query {
    aliases: Vec<AliasInfo>,
    tbl_refs: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct RuleAL05 {}

impl Rule for RuleAL05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAL05::default().erased()
    }

    fn name(&self) -> &'static str {
        "aliasing.unused"
    }

    fn description(&self) -> &'static str {
        "Tables should not be aliased if that alias is not used."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();
        let select_info = get_select_statement_info(&context.segment, context.dialect.into(), true);

        let Some(select_info) = select_info else {
            return Vec::new();
        };

        if select_info.table_aliases.is_empty() {
            return Vec::new();
        }

        let query = Query::from_segment(&context.segment, context.dialect, None);
        self.analyze_table_aliases(query.clone(), context.dialect);

        for alias in &RefCell::borrow(&query.inner).payload.aliases {
            if Self::is_alias_required(&alias.from_expression_element) {
                continue;
            }

            if alias.aliased
                && !RefCell::borrow(&query.inner).payload.tbl_refs.contains(&alias.ref_str)
            {
                let violation = self.report_unused_alias(alias.clone());
                violations.push(violation);
            }
        }

        violations
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_statement"].into()).into()
    }
}

impl RuleAL05 {
    #[allow(clippy::only_used_in_recursion)]
    fn analyze_table_aliases(&self, query: Query<AL05Query>, _dialect: &Dialect) {
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                RefCell::borrow_mut(&query.inner).payload.aliases.extend(select_info.table_aliases);

                for r in select_info.reference_buffer {
                    for tr in r.extract_possible_references(ObjectReferenceLevel::Table) {
                        Self::resolve_and_mark_reference(query.clone(), tr.part);
                    }
                }
            }
        }

        RefCell::borrow_mut(&query.inner).selectables = selectables;

        for child in query.children() {
            self.analyze_table_aliases(child, _dialect);
        }
    }

    fn resolve_and_mark_reference(query: Query<AL05Query>, r#ref: String) {
        if RefCell::borrow(&query.inner).payload.aliases.iter().any(|it| it.ref_str == r#ref) {
            RefCell::borrow_mut(&query.inner).payload.tbl_refs.push(r#ref.clone());
        } else if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
            Self::resolve_and_mark_reference(parent, r#ref);
        }
    }

    fn is_alias_required(from_expression_element: &ErasedSegment) -> bool {
        for segment in from_expression_element.iter_segments(Some(&["bracketed"]), false) {
            if segment.is_type("table_expression") {
                if segment.child(&["values_clause"]).is_some() {
                    return false;
                } else {
                    return segment.iter_segments(Some(&["bracketed"]), false).iter().any(|seg| {
                        ["select_statement", "set_expression", "with_compound_statement"]
                            .iter()
                            .any(|it| seg.is_type(it))
                    });
                }
            }
        }
        false
    }

    fn report_unused_alias(&self, alias: AliasInfo) -> LintResult {
        let mut fixes = vec![LintFix::delete(alias.alias_expression.clone().unwrap())];
        let to_delete = Segments::from_vec(alias.from_expression_element.segments().to_vec(), None)
            .reversed()
            .select(
                None,
                Some(|it| it.is_whitespace() || it.is_meta()),
                alias.alias_expression.as_ref().unwrap().into(),
                None,
            );

        fixes.extend(to_delete.into_iter().map(LintFix::delete));

        LintResult::new(
            alias.segment,
            fixes,
            None,
            format!("Alias '{}' is never used in SELECT statement.", alias.ref_str).into(),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::aliasing::AL05::RuleAL05;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAL05::default().erased()]
    }

    #[test]
    fn test_fail_table_alias_not_referenced_1() {
        let fail_str = "SELECT * FROM my_tbl AS foo";
        let fix_str = "SELECT * FROM my_tbl";

        let result = fix(fail_str.into(), rules());
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_fail_table_alias_not_referenced_1_subquery() {
        let fail_str = "SELECT * FROM (SELECT * FROM my_tbl AS foo)";
        let fix_str = "SELECT * FROM (SELECT * FROM my_tbl)";

        let result = fix(fail_str.into(), rules());
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_pass_table_alias_referenced_subquery() {
        let violations = lint(
            "SELECT * FROM (SELECT foo.bar FROM my_tbl AS foo)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_table_alias_referenced() {
        let violations = lint(
            "SELECT * FROM my_tbl AS foo JOIN other_tbl on other_tbl.x = foo.x".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_unaliased_table_referenced() {
        let violations = lint(
            "select ps.*, pandgs.blah from ps join pandgs using(moo)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_table_alias_not_referenced_2() {
        let fail_str = "SELECT * FROM my_tbl foo";
        let fix_str = "SELECT * FROM my_tbl";

        let result = fix(fail_str.into(), rules());
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_fail_table_alias_not_referenced_2_subquery() {
        let fail_str = "SELECT * FROM (SELECT * FROM my_tbl foo)";
        let fix_str = "SELECT * FROM (SELECT * FROM my_tbl)";

        let result = fix(fail_str.into(), rules());
        assert_eq!(fix_str, result);
    }

    #[test]
    fn test_pass_subquery_alias_not_referenced() {
        let violations = lint(
            "select * from (select 1 as a) subquery".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_derived_query_requires_alias_1() {
        let sql = r#"
        SELECT * FROM (
            SELECT 1
        ) as a
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_derived_query_requires_alias_2() {
        let sql = r#"
        SELECT * FROM (
            SELECT col FROM dbo.tab
            UNION
            SELECT -1 AS col
        ) AS a
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_derived_query_requires_alias_3() {
        let sql = r#"
        SELECT * FROM (
            WITH foo AS (
                SELECT col FROM dbo.tab
            )
            SELECT * FROM foo
        ) AS a
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_join_on_expression_in_parentheses() {
        let sql = r#"
        SELECT table1.c1
        FROM
            table1 AS tbl1
        INNER JOIN table2 AS tbl2 ON (tbl2.col2 = tbl1.col2)
        INNER JOIN table3 AS tbl3 ON (tbl3.col3 = tbl2.col3)
    "#;
        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_ansi_function_not_table_parameter() {
        let fail_str = r#"
            SELECT TO_JSON_STRING(t)
            FROM my_table AS t
        "#;

        let fix_str = r#"
            SELECT TO_JSON_STRING(t)
            FROM my_table
        "#;

        let result = fix(fail_str.into(), rules());
        assert_eq!(fix_str, result);
    }
}
