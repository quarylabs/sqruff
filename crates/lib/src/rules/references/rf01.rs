use std::cell::RefCell;

use ahash::AHashMap;
use itertools::Itertools;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::object_reference::{
    ObjectReferenceLevel, ObjectReferencePart, ObjectReferenceSegment,
};
use sqruff_lib_core::utils::analysis::query::{Query, Selectable};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::reference::object_ref_matches_table;

#[derive(Debug, Default, Clone)]
struct RF01Query {
    aliases: Vec<AliasInfo>,
    standalone_aliases: Vec<SmolStr>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRF01 {
    force_enable: bool,
}

impl RuleRF01 {
    #[allow(clippy::only_used_in_recursion)]
    fn resolve_reference(
        &self,
        r: &ObjectReferenceSegment,
        tbl_refs: Vec<(ObjectReferencePart, Vec<SmolStr>)>,
        dml_target_table: &[SmolStr],
        query: Query<RF01Query>,
    ) -> Option<LintResult> {
        let possible_references: Vec<_> = tbl_refs
            .clone()
            .into_iter()
            .map(|tbl_ref| tbl_ref.1)
            .collect();

        let mut targets = vec![];

        for alias in &RefCell::borrow(&query.inner).payload.aliases {
            if alias.aliased {
                targets.push(vec![alias.ref_str.clone()]);
            }

            if let Some(object_reference) = &alias.object_reference {
                let references = object_reference
                    .reference()
                    .iter_raw_references()
                    .into_iter()
                    .map(|it| it.part.into())
                    .collect_vec();

                targets.push(references);
            }
        }

        for standalone_alias in &RefCell::borrow(&query.inner).payload.standalone_aliases {
            targets.push(vec![standalone_alias.clone()]);
        }

        if !object_ref_matches_table(&possible_references, &targets) {
            if let Some(parent) = RefCell::borrow(&query.inner).parent.clone() {
                return self.resolve_reference(r, tbl_refs.clone(), dml_target_table, parent);
            } else if dml_target_table.is_empty()
                || !object_ref_matches_table(&possible_references, &[dml_target_table.to_vec()])
            {
                return LintResult::new(
                    tbl_refs[0].0.segments[0].clone().into(),
                    Vec::new(),
                    format!(
                        "Reference '{}' refers to table/view not found in the FROM clause or \
                         found in ancestor statement.",
                        r.0.raw()
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
        r: &ObjectReferenceSegment,
        dialect: &Dialect,
    ) -> Vec<(ObjectReferencePart, Vec<SmolStr>)> {
        let mut tbl_refs = Vec::new();

        for values in r.extract_possible_multipart_references(&[
            ObjectReferenceLevel::Schema,
            ObjectReferenceLevel::Table,
        ]) {
            tbl_refs.push((
                values[1].clone(),
                vec![values[0].part.clone().into(), values[1].part.clone().into()],
            ));
        }

        if tbl_refs.is_empty() || dialect.name == DialectKind::Bigquery {
            tbl_refs.extend(
                r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name)
                    .into_iter()
                    .map(|it| (it.clone(), vec![it.part.into()])),
            );
        }

        tbl_refs
    }

    fn analyze_table_references(
        &self,
        query: Query<RF01Query>,
        dml_target_table: &[SmolStr],
        violations: &mut Vec<LintResult>,
    ) {
        let selectables = std::mem::take(&mut RefCell::borrow_mut(&query.inner).selectables);

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                RefCell::borrow_mut(&query.inner)
                    .payload
                    .aliases
                    .extend(select_info.table_aliases);
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
        reference: &ObjectReferenceSegment,
        selectable: &Selectable,
    ) -> bool {
        let ref_path = selectable.selectable.path_to(&reference.0);

        if !ref_path.is_empty() {
            ref_path
                .iter()
                .any(|ps| ps.segment.is_type(SyntaxKind::IntoTableClause))
        } else {
            false
        }
    }
}

impl Rule for RuleRF01 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleRF01 {
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.from"
    }

    fn description(&self) -> &'static str {
        "References cannot reference objects not present in 'FROM' clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the reference `vee` has not been declared.

```sql
SELECT
    vee.a
FROM foo
```

**Best practice**

Remove the reference.

```sql
SELECT
    a
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::References]
    }

    fn force_enable(&self) -> bool {
        self.force_enable
    }

    fn dialect_skip(&self) -> &'static [DialectKind] {
        // TODO Add others when finished, whole list["databricks", "hive", "soql"]
        &[
            DialectKind::Redshift,
            DialectKind::Bigquery,
            DialectKind::Sparksql,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let query = Query::from_segment(&context.segment, context.dialect, None);
        let mut violations = Vec::new();
        let tmp;

        let dml_target_table = if !context.segment.is_type(SyntaxKind::SelectStatement) {
            let refs = context.segment.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::TableReference]) },
                true,
                &SyntaxSet::EMPTY,
                true,
            );
            if let Some(reference) = refs.first() {
                let reference = reference.reference();

                tmp = reference
                    .iter_raw_references()
                    .into_iter()
                    .map(|it| it.part.into())
                    .collect_vec();
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
            const {
                SyntaxSet::new(&[
                    SyntaxKind::DeleteStatement,
                    SyntaxKind::MergeStatement,
                    SyntaxKind::SelectStatement,
                    SyntaxKind::UpdateStatement,
                ])
            },
        )
        .disallow_recurse()
        .into()
    }
}
