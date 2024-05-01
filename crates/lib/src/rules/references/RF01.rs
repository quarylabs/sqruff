use std::cell::RefCell;

use ahash::AHashMap;
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::parser::segments::base::{Segment, SegmentExt};
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::reference::object_ref_matches_table;
use crate::dialects::ansi::{
    Node, ObjectReferenceLevel, ObjectReferencePart, ObjectReferenceSegment, TableReferenceSegment,
};
use crate::helpers::ToErasedSegment;
use crate::utils::analysis::query::{Query, Selectable};

#[derive(Debug, Default, Clone)]
struct RF01Query {
    aliases: Vec<AliasInfo>,
    standalone_aliases: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRF01;

impl RuleRF01 {
    #[allow(clippy::only_used_in_recursion)]
    fn resolve_reference(
        &self,
        r: &Node<ObjectReferenceSegment>,
        tbl_refs: Vec<(ObjectReferencePart, Vec<String>)>,
        dml_target_table: &[String],
        query: Query<RF01Query>,
    ) -> Option<LintResult> {
        let possible_references: Vec<_> =
            tbl_refs.clone().into_iter().map(|tbl_ref| tbl_ref.1).collect();

        let mut targets = vec![];

        for alias in &RefCell::borrow(&query.inner).payload.aliases {
            if alias.aliased {
                targets.push(vec![alias.ref_str.clone()]);
            }

            if let Some(object_reference) = &alias.object_reference {
                let references = object_reference
                    .as_any()
                    .downcast_ref::<Node<ObjectReferenceSegment>>()
                    .unwrap()
                    .iter_raw_references()
                    .into_iter()
                    .map(|it| it.part)
                    .collect_vec();

                targets.push(references);
            }
        }

        for standalone_alias in &RefCell::borrow(&query.inner).payload.standalone_aliases {
            targets.push(vec![standalone_alias.clone()]);
        }

        if !object_ref_matches_table(possible_references.clone(), targets) {
            if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
                return self.resolve_reference(r, tbl_refs.clone(), dml_target_table, parent);
            } else if dml_target_table.is_empty()
                || !object_ref_matches_table(possible_references, vec![dml_target_table.to_vec()])
            {
                return LintResult::new(
                    tbl_refs[0].0.segments[0].clone().into(),
                    Vec::new(),
                    None,
                    format!(
                        "Reference '{}' refers to table/view not found in the FROM clause or \
                         found in ancestor statement.",
                        r.get_raw().unwrap()
                    )
                    .into(),
                    None,
                )
                .into();
            }
        }
        None
    }

    fn get_table_refs(
        &self,
        r: &Node<ObjectReferenceSegment>,
        _dialect: &Dialect,
    ) -> Vec<(ObjectReferencePart, Vec<String>)> {
        let mut tbl_refs = Vec::new();

        for values in r.extract_possible_multipart_references(&[
            ObjectReferenceLevel::Schema,
            ObjectReferenceLevel::Table,
        ]) {
            tbl_refs
                .push((values[1].clone(), vec![values[0].part.clone(), values[1].part.clone()]));
        }

        if tbl_refs.is_empty() {
            tbl_refs.extend(
                r.extract_possible_references(ObjectReferenceLevel::Table)
                    .into_iter()
                    .map(|it| (it.clone(), vec![it.part])),
            );
        }

        tbl_refs
    }

    fn analyze_table_references(
        &self,
        query: Query<RF01Query>,
        dml_target_table: &[String],
        violations: &mut Vec<LintResult>,
    ) {
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                RefCell::borrow_mut(&query.inner).payload.aliases.extend(select_info.table_aliases);
                RefCell::borrow_mut(&query.inner)
                    .payload
                    .standalone_aliases
                    .extend(select_info.standalone_aliases);

                for r in select_info.reference_buffer {
                    if !self.should_ignore_reference(&r, selectable) {
                        let violation = self.resolve_reference(
                            &r,
                            self.get_table_refs(&r, RefCell::borrow(&query.inner).dialect),
                            dml_target_table,
                            query.clone(),
                        );
                        violations.extend(violation);
                    }
                }
            }
        }

        RefCell::borrow_mut(&query.inner).selectables = selectables;

        for child in query.children() {
            self.analyze_table_references(child, dml_target_table, violations);
        }
    }

    fn should_ignore_reference(
        &self,
        reference: &Node<ObjectReferenceSegment>,
        selectable: &Selectable,
    ) -> bool {
        let ref_path = selectable.selectable.path_to(&reference.clone().to_erased_segment());

        if !ref_path.is_empty() {
            ref_path.iter().any(|ps| ps.segment.is_type("into_table_clause"))
        } else {
            false
        }
    }
}

impl Rule for RuleRF01 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleRF01.erased()
    }

    fn name(&self) -> &'static str {
        "references.from"
    }

    fn description(&self) -> &'static str {
        "References cannot reference objects not present in 'FROM' clause."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let query = Query::from_segment(&context.segment, context.dialect, None);
        let mut violations = Vec::new();
        let tmp;

        let dml_target_table = if !context.segment.is_type("select_statement") {
            let refs = context.segment.recursive_crawl(&["table_reference"], true, None, true);
            if let Some(reference) = refs.first() {
                let mut node = Node::new();

                let reference = if reference.is_type("table_reference") {
                    let tr =
                        reference.as_any().downcast_ref::<Node<TableReferenceSegment>>().unwrap();
                    node.segments.clone_from(&tr.segments);
                    &node
                } else {
                    reference.as_any().downcast_ref::<Node<ObjectReferenceSegment>>().unwrap()
                };

                tmp = reference.iter_raw_references().into_iter().map(|it| it.part).collect_vec();
                &tmp
            } else {
                [].as_slice()
            }
        } else {
            &[]
        };

        self.analyze_table_references(query, dml_target_table, &mut violations);

        violations
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            ["delete_statement", "merge_statement", "select_statement", "update_statement"].into(),
        )
        .disallow_recurse()
        .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::references::RF01::RuleRF01;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleRF01.erased()]
    }

    #[test]
    fn test_fail_object_not_referenced_2() {
        let violations = lint(
            "SELECT * FROM my_tbl WHERE foo.bar > 0".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Reference 'foo.bar' refers to table/view not found in the FROM clause or found in \
             ancestor statement."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 28);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_object_referenced_2() {
        let violations = lint(
            "SELECT * FROM db.sc.tbl2 WHERE a NOT IN (SELECT a FROM db.sc.tbl1)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_object_referenced_3() {
        let violations = lint(
            "SELECT * FROM db.sc.tbl2 WHERE a NOT IN (SELECT tbl2.a FROM db.sc.tbl1)".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_object_referenced_5b() {
        let violations =
            lint("SELECT col1.field FROM table1".into(), "ansi".into(), rules(), None, None)
                .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Reference 'col1.field' refers to table/view not found in the FROM clause or found in \
             ancestor statement."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 8);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_object_referenced_6() {
        let violations = lint(
            "select cc.c1 from (select table1.c1 from table1 inner join table2 on table1.x_id = \
             table2.x_id inner join table3 on table2.y_id = table3.y_id) as cc"
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
    fn test_pass_object_referenced_7() {
        let violations = lint(
            "UPDATE my_table SET row_sum = (SELECT COUNT(*) AS row_sum FROM another_table WHERE \
             another_table.id = my_table.id)"
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
    fn test_fail_object_referenced_7() {
        let violations = lint(
            "UPDATE my_table SET row_sum = (SELECT COUNT(*) AS row_sum FROM another_table WHERE \
             another_table.id = my_tableeee.id)"
                .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Reference 'my_tableeee.id' refers to table/view not found in the FROM clause or \
             found in ancestor statement."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 103);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_object_referenced_8() {
        let violations = lint(
            "DELETE FROM agent1 WHERE EXISTS (SELECT customer.cust_id FROM customer WHERE \
             agent1.agent_code <> customer.agent_code)"
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
    fn test_pass_two_part_reference_8() {
        let violations = lint(
            "delete from public.agent1 where exists (select customer.cust_id from customer where \
             agent1.agent_code <> customer.agent_code)"
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
    fn test_pass_two_part_reference_9() {
        let violations = lint(
            "delete from public.agent1 where exists (select customer.cust_id from customer where \
             public.agent1.agent_code <> customer.agent_code)"
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
    fn test_fail_two_part_reference_10() {
        let violations = lint(
            "select * from schema1.agent1 where schema2.agent1.agent_code <> 'abc'".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Reference 'schema2.agent1.agent_code' refers to table/view not found in the FROM \
             clause or found in ancestor statement."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 44);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_two_part_reference_11() {
        let violations = lint(
            "delete from schema1.agent1 where exists (select customer.cust_id from customer where \
             schema2.agent1.agent_code <> customer.agent_code)"
                .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Reference 'schema2.agent1.agent_code' refers to table/view not found in the FROM \
             clause or found in ancestor statement."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 94);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_two_part_reference_11() {
        let violations = lint(
            "select * from agent1 where public.agent1.agent_code <> '3'".into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_simple_delete() {
        let violations =
            lint("delete from table1 where 1 = 1".into(), "ansi".into(), rules(), None, None)
                .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_update_with_alias() {
        let violations = lint(
            "UPDATE tbl AS dest SET t.title = 'TEST' WHERE t.id = 101 AND EXISTS (SELECT 1 FROM \
             foobar AS tmp WHERE tmp.idx = dest.idx)"
                .into(),
            "ansi".into(),
            rules(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }
}
