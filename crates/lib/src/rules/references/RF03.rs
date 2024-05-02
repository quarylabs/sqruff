use std::cell::RefCell;

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::core::parser::segments::base::{
    ErasedSegment, IdentifierSegment, Segment, SymbolSegment,
};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::{Node, ObjectReferenceSegment};
use crate::helpers::{capitalize, ToErasedSegment};
use crate::utils::analysis::query::Query;

#[derive(Debug, Clone)]
pub struct RuleRF03 {
    single_table_references: String,
}

impl Default for RuleRF03 {
    fn default() -> Self {
        Self { single_table_references: "consistent".into() }
    }
}

impl RuleRF03 {
    #[allow(clippy::only_used_in_recursion)]
    fn visit_queries(
        &self,
        query: Query<()>,
        visited: &mut AHashSet<ErasedSegment>,
    ) -> Vec<LintResult> {
        #[allow(unused_assignments)]
        let mut select_info = None;

        let mut acc = Vec::new();
        let selectables = &RefCell::borrow(&query.inner).selectables;

        if !selectables.is_empty() {
            select_info = selectables[0].select_info();

            if let Some(select_info) = select_info.clone()
                && select_info.table_aliases.len() == 1
            {
                let mut fixable = true;
                let possible_ref_tables = iter_available_targets(query.clone());

                if let Some(_parent) = &RefCell::borrow(&query.inner).parent {}

                if possible_ref_tables.len() > 1 {
                    fixable = false;
                }

                let results = check_references(
                    select_info.table_aliases,
                    select_info.standalone_aliases,
                    select_info.reference_buffer,
                    select_info.col_aliases,
                    &self.single_table_references,
                    false,
                    Some("qualified".into()),
                    fixable,
                );

                acc.extend(results);
            }
        }

        let children = query.children();

        for child in children {
            acc.extend(self.visit_queries(child, visited));
        }

        acc
    }
}

fn iter_available_targets(query: Query<()>) -> Vec<String> {
    RefCell::borrow(&query.inner)
        .selectables
        .iter()
        .flat_map(|selectable| {
            selectable
                .select_info()
                .unwrap()
                .table_aliases
                .iter()
                .map(|alias| alias.ref_str.clone())
                .collect_vec()
        })
        .collect_vec()
}

#[allow(clippy::too_many_arguments)]
fn check_references(
    table_aliases: Vec<AliasInfo>,
    standalone_aliases: Vec<String>,
    references: Vec<Node<ObjectReferenceSegment>>,
    col_aliases: Vec<ColumnAliasInfo>,
    single_table_references: &str,
    is_struct_dialect: bool,
    fix_inconsistent_to: Option<String>,
    fixable: bool,
) -> Vec<LintResult> {
    let mut acc = Vec::new();

    let col_alias_names =
        col_aliases.clone().into_iter().map(|it| it.alias_identifier_name).collect_vec();

    let table_ref_str = &table_aliases[0].ref_str;
    let table_ref_str_source = table_aliases[0].segment.clone();
    let mut seen_ref_types = AHashSet::new();

    for reference in references.clone() {
        let this_ref_type = reference.qualification();
        if this_ref_type == "qualified" && is_struct_dialect {
            unimplemented!()
        }

        let lint_res = validate_one_reference(
            single_table_references,
            reference,
            this_ref_type,
            &standalone_aliases,
            table_ref_str,
            table_ref_str_source.clone(),
            &col_alias_names,
            &seen_ref_types,
            fixable,
        );

        seen_ref_types.insert(this_ref_type);
        let Some(lint_res) = lint_res else {
            continue;
        };

        if let Some(fix_inconsistent_to) = &fix_inconsistent_to
            && single_table_references == "consistent"
        {
            let results = check_references(
                table_aliases.clone(),
                standalone_aliases.clone(),
                references.clone(),
                col_aliases.clone(),
                fix_inconsistent_to,
                is_struct_dialect,
                None,
                fixable,
            );

            acc.extend(results);
        }

        acc.push(lint_res);
    }

    acc
}

#[allow(clippy::too_many_arguments)]
fn validate_one_reference(
    single_table_references: &str,
    ref_: Node<ObjectReferenceSegment>,
    this_ref_type: &str,
    standalone_aliases: &[String],
    table_ref_str: &str,
    _table_ref_str_source: Option<ErasedSegment>,
    col_alias_names: &[String],
    seen_ref_types: &AHashSet<&str>,
    fixable: bool,
) -> Option<LintResult> {
    if !ref_.is_qualified() && ref_.is_type("wildcard_identifier") {
        return None;
    }

    if standalone_aliases.contains(&ref_.get_raw().unwrap()) {
        return None;
    }

    if table_ref_str.is_empty() {
        return None;
    }

    if col_alias_names.contains(&ref_.get_raw().unwrap()) {
        return None;
    }

    if single_table_references == "consistent" {
        return if !seen_ref_types.is_empty() && !seen_ref_types.contains(this_ref_type) {
            LintResult::new(
                ref_.clone().to_erased_segment().into(),
                Vec::new(),
                None,
                format!(
                    "{} reference '{}' found in single table select which is inconsistent with \
                     previous references.",
                    capitalize(this_ref_type),
                    ref_.get_raw().unwrap()
                )
                .into(),
                None,
            )
            .into()
        } else {
            None
        };
    }

    if single_table_references == this_ref_type {
        return None;
    }

    if single_table_references == "unqualified" {
        let fixes = if fixable {
            ref_.segments.iter().take(2).cloned().map(LintFix::delete).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        return LintResult::new(
            ref_.clone().to_erased_segment().into(),
            fixes,
            None,
            format!(
                "{} reference '{}' found in single table select.",
                capitalize(this_ref_type),
                ref_.get_raw().unwrap()
            )
            .into(),
            None,
        )
        .into();
    }

    let ref_ = ref_.to_erased_segment();
    let fixes = if fixable {
        vec![LintFix::create_before(
            if ref_.segments().is_empty() { ref_.segments()[0].clone() } else { ref_.clone() },
            vec![
                IdentifierSegment::create(table_ref_str, &<_>::default(), <_>::default()),
                SymbolSegment::create(".", &<_>::default(), <_>::default()),
            ],
        )]
    } else {
        Vec::new()
    };

    LintResult::new(
        ref_.clone().into(),
        fixes,
        None,
        format!(
            "{} reference '{}' found in single table select.",
            capitalize(this_ref_type),
            ref_.get_raw().unwrap()
        )
        .into(),
        None,
    )
    .into()
}

impl Rule for RuleRF03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleRF03::default().erased()
    }

    fn name(&self) -> &'static str {
        "references.consistent"
    }

    fn description(&self) -> &'static str {
        "References should be consistent in statements with a single table."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let query: Query<()> = Query::from_segment(&context.segment, context.dialect, None);
        let mut visited: AHashSet<ErasedSegment> = AHashSet::new();

        self.visit_queries(query, &mut visited)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            ["select_statement", "set_expression", "with_compound_statement"].into(),
        )
        .disallow_recurse()
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::RuleRF03;
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules() -> Vec<ErasedRule> {
        vec![RuleRF03::default().erased()]
    }

    fn rules_unqualified() -> Vec<ErasedRule> {
        vec![RuleRF03 { single_table_references: "unqualified".into() }.erased()]
    }

    fn rules_qualified() -> Vec<ErasedRule> {
        vec![RuleRF03 { single_table_references: "qualified".into() }.erased()]
    }

    #[test]
    fn test_fail_single_table_mixed_qualification_of_references() {
        let fail_str = "SELECT my_tbl.bar, baz FROM my_tbl";
        let fix_str = "SELECT my_tbl.bar, my_tbl.baz FROM my_tbl";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_pass_single_table_consistent_references_1() {
        let violations =
            lint("SELECT bar FROM my_tbl".into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_single_table_consistent_references_2() {
        let violations =
            lint("SELECT my_tbl.bar FROM my_tbl".into(), "ansi".into(), rules(), None, None)
                .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_on_tableless_table() {
        let violations = lint(
            "SELECT (SELECT MAX(bar) FROM tbl) + 1 AS col".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_single_table_mixed_qualification_of_references_subquery() {
        let fail_str = "SELECT * FROM (SELECT my_tbl.bar, baz FROM my_tbl)";
        let fix_str = "SELECT * FROM (SELECT my_tbl.bar, my_tbl.baz FROM my_tbl)";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_pass_lateral_table_ref() {
        let violations = lint(
            "SELECT tbl.a, tbl.b, tbl.a + tbl.b AS col_created_right_here, col_created_right_here \
             + 1 AS sub_self_ref FROM tbl"
                .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_single_table_consistent_references_1_subquery() {
        let violations = lint(
            "SELECT * FROM (SELECT bar FROM my_tbl)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_single_table_consistent_references_2_subquery() {
        let violations = lint(
            "SELECT * FROM (SELECT my_tbl.bar FROM my_tbl)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_single_table_reference_when_unqualified_config() {
        let fail_str = "SELECT my_tbl.bar FROM my_tbl";
        let fix_str = "SELECT bar FROM my_tbl";

        let actual = fix(fail_str.into(), rules_unqualified());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_fail_single_table_reference_when_qualified_config() {
        let fail_str = "SELECT bar FROM my_tbl WHERE foo";
        let fix_str = "SELECT my_tbl.bar FROM my_tbl WHERE my_tbl.foo";

        let actual = fix(fail_str.into(), rules_qualified());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_pass_single_table_reference_in_subquery() {
        let pass_str = "SELECT * FROM db.sc.tbl2 WHERE a NOT IN (SELECT a FROM db.sc.tbl1)";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_object_references_1a() {
        let fail_str = "SELECT a.bar, b FROM my_tbl";
        let fix_str = "SELECT a.bar, my_tbl.b FROM my_tbl";

        let actual = fix(fail_str.into(), rules());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_pass_group_by_alias() {
        let pass_str =
            "select t.col1 + 1 as alias_col1, count(1) from table1 as t group by alias_col1";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_select_alias_in_where_clause_5() {
        let fail_str =
            "select t.col0, t.col1 + 1 as alias_col1 from table1 as t where alias_col1 > 5";
        let fix_str = "select col0, col1 + 1 as alias_col1 from table1 as t where alias_col1 > 5";

        let actual = fix(fail_str.into(), rules_unqualified());
        assert_eq!(actual, fix_str);
    }

    #[test]
    fn test_unfixable_ambiguous_reference_subquery() {
        let fail_str = "SELECT (SELECT other_table.other_table_field_1 FROM other_table WHERE \
                        other_table.id = field_2) FROM (SELECT * FROM some_table) AS my_alias";

        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(
            violations[0].desc(),
            "Unqualified reference 'field_2' found in single table select."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 88);

        assert_eq!(
            violations[1].desc(),
            "Unqualified reference 'field_2' found in single table select which is inconsistent \
             with previous references."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 88);
    }
}
