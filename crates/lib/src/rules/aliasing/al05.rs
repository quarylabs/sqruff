use ahash::{AHashMap, AHashSet};
use smol_str::{SmolStr, ToSmolStr};
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceLevel;
use sqruff_lib_core::utils::analysis::query::Query;
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Default, Clone)]
struct State {
    aliases: Vec<AliasInfo>,
    tbl_refs: Vec<SmolStr>,
}

#[derive(Debug, Default, Clone)]
pub struct RuleAL05;

impl Rule for RuleAL05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL05.erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.unused"
    }

    fn description(&self) -> &'static str {
        "Tables should not be aliased if that alias is not used."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias `zoo` is not used.

```sql
SELECT
    a
FROM foo AS zoo
```

**Best practice**

Use the alias or remove it. An unused alias makes code harder to read without changing any functionality.

```sql
SELECT
    zoo.a
FROM foo AS zoo

-- Alternatively...

SELECT
    a
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();
        let select_info = get_select_statement_info(&context.segment, context.dialect.into(), true);

        let Some(select_info) = select_info else {
            return Vec::new();
        };

        if select_info.table_aliases.is_empty() {
            return Vec::new();
        }

        let query = Query::from_segment(&context.segment, context.dialect);
        let mut state_stack = vec![State::default()];
        self.analyze_table_aliases(query.clone(), context.dialect, &mut state_stack);
        let state = state_stack.pop().unwrap();

        if context.dialect.name == DialectKind::Redshift {
            let mut references = AHashSet::default();
            let mut aliases = AHashSet::default();

            for alias in &state.aliases {
                aliases.insert(alias.ref_str.clone());
                if let Some(object_reference) = &alias.object_reference {
                    for seg in object_reference.segments() {
                        if const {
                            SyntaxSet::new(&[
                                SyntaxKind::Identifier,
                                SyntaxKind::NakedIdentifier,
                                SyntaxKind::QuotedIdentifier,
                                SyntaxKind::ObjectReference,
                            ])
                        }
                        .contains(seg.get_type())
                        {
                            references.insert(seg.raw().to_smolstr());
                        }
                    }
                }
            }

            if aliases.intersection(&references).next().is_some() {
                return Vec::new();
            }
        }

        for alias in &state.aliases {
            if Self::is_alias_required(&alias.from_expression_element, context.dialect.name) {
                continue;
            }

            if alias.aliased && !state.tbl_refs.contains(&alias.ref_str) {
                let violation = self.report_unused_alias(alias.clone());
                violations.push(violation);
            }
        }

        violations
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

impl RuleAL05 {
    #[allow(clippy::only_used_in_recursion)]
    fn analyze_table_aliases(&self, query: Query, dialect: &Dialect, state_stack: &mut Vec<State>) {
        let selectables = query.selectables.clone();

        for selectable in &selectables {
            if let Some(select_info) = selectable.select_info() {
                state_stack
                    .last_mut()
                    .unwrap()
                    .aliases
                    .extend(select_info.table_aliases);

                for r in select_info.reference_buffer {
                    for tr in
                        r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name)
                    {
                        Self::resolve_and_mark_reference(state_stack, tr.part);
                    }
                }
            }
        }

        for child in query.children() {
            state_stack.push(State::default());
            self.analyze_table_aliases(child, dialect, state_stack);
            state_stack.pop();
        }
    }

    fn resolve_and_mark_reference(state_stack: &mut Vec<State>, r#ref: String) {
        for st in state_stack.iter_mut().rev() {
            if st.aliases.iter().any(|it| it.ref_str == r#ref) {
                st.tbl_refs.push(r#ref.into());
                return;
            }
        }
    }

    fn is_alias_required(
        from_expression_element: &ErasedSegment,
        dialect_name: DialectKind,
    ) -> bool {
        for segment in from_expression_element
            .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
        {
            if segment.is_type(SyntaxKind::TableExpression) {
                return if segment
                    .child(const { &SyntaxSet::new(&[SyntaxKind::ValuesClause]) })
                    .is_some()
                {
                    matches!(dialect_name, DialectKind::Snowflake)
                } else {
                    segment
                        .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
                        .iter()
                        .any(|seg| {
                            const {
                                SyntaxSet::new(&[
                                    SyntaxKind::SelectStatement,
                                    SyntaxKind::SetExpression,
                                    SyntaxKind::WithCompoundStatement,
                                ])
                            }
                            .contains(seg.get_type())
                        })
                };
            }
        }
        false
    }

    fn report_unused_alias(&self, alias: AliasInfo) -> LintResult {
        let mut fixes = vec![LintFix::delete(alias.alias_expression.clone().unwrap())];
        let to_delete = Segments::from_vec(alias.from_expression_element.segments().to_vec(), None)
            .reversed()
            .select::<fn(&ErasedSegment) -> bool>(
                None,
                Some(|it| it.is_whitespace() || it.is_meta()),
                alias.alias_expression.as_ref().unwrap().into(),
                None,
            );

        fixes.extend(to_delete.into_iter().map(LintFix::delete));

        LintResult::new(
            alias.segment,
            fixes,
            format!(
                "Alias '{}' is never used in SELECT statement.",
                alias.ref_str
            )
            .into(),
            None,
        )
    }
}
