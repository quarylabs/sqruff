use ahash::AHashMap;
use itertools::Itertools;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::object_reference::{
    ObjectReferenceLevel, ObjectReferencePart, ObjectReferenceSegment,
};
use sqruff_lib_core::utils::analysis::query::{Query, Selectable};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::reference::object_ref_matches_table;
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
struct State {
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
        state_stack: &[State],
    ) -> Option<LintResult> {
        let possible_references: Vec<_> = tbl_refs
            .clone()
            .into_iter()
            .map(|tbl_ref| tbl_ref.1)
            .collect();

        let mut targets = vec![];
        for st in state_stack.iter().rev() {
            for alias in &st.aliases {
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

            for standalone_alias in &st.standalone_aliases {
                targets.push(vec![standalone_alias.clone()]);
            }
        }

        if !object_ref_matches_table(&possible_references, &targets) {
            if dml_target_table.is_empty()
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
        query: Query,
        dml_target_table: &[SmolStr],
        violations: &mut Vec<LintResult>,
        state_stack: &mut Vec<State>,
    ) {
        let selectables = query.selectables.clone();
        let mut state = State::default();

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                state.aliases.extend(select_info.table_aliases);
                state
                    .standalone_aliases
                    .extend(select_info.standalone_aliases);

                for r in select_info.reference_buffer {
                    if !self.should_ignore_reference(&r, selectable) {
                        let dialect = query.dialect;
                        state_stack.push(state.clone());
                        let violation = self.resolve_reference(
                            &r,
                            self.get_table_refs(&r, dialect),
                            dml_target_table,
                            state_stack,
                        );
                        state_stack.pop();
                        violations.extend(violation);
                    }
                }
            }
        }

        for child in query.children() {
            state_stack.push(state.clone());
            self.analyze_table_references(child, dml_target_table, violations, state_stack);
            state_stack.pop();
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
        let query = Query::from_segment(&context.segment, context.dialect);
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

        let mut state_stack = Vec::new();
        self.analyze_table_references(query, dml_target_table, &mut violations, &mut state_stack);

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
