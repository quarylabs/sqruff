use std::cell::RefCell;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use smol_str::SmolStr;
use sqruff_lib_core::dialects::common::{AliasInfo, ColumnAliasInfo};
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::capitalize;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::from::FromClauseSegment;
use sqruff_lib_core::parser::segments::object_reference::{
    ObjectReferenceLevel, ObjectReferenceSegment,
};
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::analysis::query::{Query, Selectable};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

use super::postgres_order_by_alias_references;

#[derive(Debug, Clone, Default)]
pub struct RuleRF03 {
    single_table_references: Option<String>,
    force_enable: bool,
}

impl RuleRF03 {
    fn visit_queries(
        tables: &Tables,
        dialect_name: DialectKind,
        single_table_references: &str,
        is_struct_dialect: bool,
        query: Query<'_>,
        _visited: &mut HashSet<ErasedSegment>,
    ) -> Vec<LintResult> {
        #[allow(unused_assignments)]
        let mut select_info = None;

        let mut acc = Vec::new();
        let selectables = &RefCell::borrow(&query.inner).selectables;

        if !selectables.is_empty() {
            select_info = selectables[0].select_info();

            if let Some(mut select_info) = select_info
                .clone()
                .filter(|select_info| select_info.table_aliases.len() == 1)
            {
                // :TRICKY: Subqueries in the column list of a SELECT can see tables
                // in the FROM list of the containing query. Thus, count tables
                // at the *parent* query level. Only check if it is a subquery of the
                // parent.
                let mut possible_ref_tables = iter_available_targets(query.clone(), None);
                let is_subquery = RefCell::borrow(&query.inner).is_subquery;
                if is_subquery && let Some(parent) = query.parent() {
                    possible_ref_tables.extend(iter_available_targets(parent, Some(query.clone())));
                }

                // A bare column can resolve to a visible enclosing source.
                let fixable = possible_ref_tables.len() <= 1
                    && (single_table_references == "unqualified"
                        || !has_visible_outer_sources(query.clone()));

                let order_by_aliases =
                    postgres_order_by_alias_references(&select_info.select_statement, dialect_name);
                if !order_by_aliases.is_empty() {
                    select_info
                        .reference_buffer
                        .retain(|reference| !order_by_aliases.contains(&reference.0.id()));
                }

                let results = check_references(
                    tables,
                    dialect_name,
                    select_info.table_aliases,
                    select_info.standalone_aliases,
                    select_info.reference_buffer,
                    select_info.col_aliases,
                    single_table_references,
                    is_struct_dialect,
                    Some("qualified".into()),
                    fixable,
                );

                acc.extend(results);
            }
        }

        let children = query.children();
        for child in children {
            acc.extend(Self::visit_queries(
                tables,
                dialect_name,
                single_table_references,
                is_struct_dialect,
                child,
                _visited,
            ));
        }

        acc
    }
}

fn has_visible_outer_sources(mut child: Query<'_>) -> bool {
    while let Some(parent) = child.parent() {
        let child_select = {
            let inner = child.inner.borrow();
            inner
                .selectables
                .first()
                .filter(|_| inner.is_subquery)
                .map(|selectable| selectable.selectable.clone())
        };

        if let Some(child_select) = child_select
            && parent
                .inner
                .borrow()
                .selectables
                .iter()
                .any(|outer| has_visible_parent_sources(outer, &child_select))
        {
            return true;
        }

        child = parent;
    }

    false
}

fn has_visible_parent_sources(outer: &Selectable<'_>, child: &ErasedSegment) -> bool {
    let path = outer.selectable.path_to(child);
    if path.is_empty() {
        return false;
    }

    if let Some(index) = path
        .iter()
        .rposition(|step| step.segment.is_type(SyntaxKind::FromExpressionElement))
    {
        let (outer_path, item_path) = path.split_at(index);
        let item = &item_path[0].segment;
        let is_apply = outer_path.last().is_some_and(|step| {
            step.segment.is_type(SyntaxKind::JoinClause) && has_keyword(&step.segment, "APPLY")
        });
        let is_table_function_argument = item_path.windows(2).any(|steps| {
            steps[0].segment.is_type(SyntaxKind::TableExpression)
                && steps[1].segment.is_type(SyntaxKind::Function)
        });
        return (has_keyword(item, "LATERAL") || is_apply || is_table_function_argument)
            && has_preceding_from_source(&outer.selectable, item);
    }

    outer.select_info().is_some_and(|info| {
        !info.standalone_aliases.is_empty()
            || info
                .table_aliases
                .iter()
                .any(|alias| alias.from_expression_element.path_to(child).is_empty())
    })
}

fn has_keyword(segment: &ErasedSegment, keyword: &str) -> bool {
    segment
        .children(const { &SyntaxSet::single(SyntaxKind::Keyword) })
        .any(|child| child.raw().eq_ignore_ascii_case(keyword))
}

fn has_preceding_from_source(select: &ErasedSegment, item: &ErasedSegment) -> bool {
    let Some(from_clause) = select.child(const { &SyntaxSet::single(SyntaxKind::FromClause) })
    else {
        return false;
    };
    let item_position = item.get_position_marker();

    // FROM aliases can come from ordinary tables or value-table functions.
    // Compare positions in the rendered SQL, where scope is resolved.
    FromClauseSegment(from_clause)
        .eventual_aliases()
        .iter()
        .any(|(_, alias)| {
            let source = &alias.from_expression_element;
            source.id() != item.id()
                && source
                    .get_position_marker()
                    .zip(item_position)
                    .is_none_or(|(source, item)| {
                        source.templated_slice.end <= item.templated_slice.start
                    })
        })
}

fn iter_available_targets(query: Query<'_>, subquery: Option<Query<'_>>) -> Vec<AliasInfo> {
    let subquery_selectable = subquery.as_ref().and_then(|subquery| {
        RefCell::borrow(&subquery.inner)
            .selectables
            .first()
            .map(|selectable| selectable.selectable.clone())
    });
    let has_subquery = subquery.is_some();

    RefCell::borrow(&query.inner)
        .selectables
        .iter()
        .flat_map(|selectable| {
            selectable
                .select_info()
                .into_iter()
                .flat_map(|select_info| {
                    select_info
                        .table_aliases
                        .into_iter()
                        .filter(|alias| {
                            if let Some(subquery_selectable) = &subquery_selectable
                                && !alias
                                    .from_expression_element
                                    .path_to(subquery_selectable)
                                    .is_empty()
                            {
                                // Skip the subquery alias itself.
                                return false;
                            }

                            (has_subquery && alias.object_reference.is_none())
                                || !alias.ref_str.is_empty()
                        })
                        .collect_vec()
                })
                .collect_vec()
        })
        .collect_vec()
}

#[allow(clippy::too_many_arguments)]
fn check_references(
    tables: &Tables,
    dialect_name: DialectKind,
    table_aliases: Vec<AliasInfo>,
    standalone_aliases: Vec<SmolStr>,
    references: Vec<ObjectReferenceSegment>,
    col_aliases: Vec<ColumnAliasInfo>,
    single_table_references: &str,
    is_struct_dialect: bool,
    fix_inconsistent_to: Option<String>,
    fixable: bool,
) -> Vec<LintResult> {
    let mut acc = Vec::new();

    let col_alias_names = col_aliases
        .clone()
        .into_iter()
        .map(|it| it.alias_identifier_name)
        .collect_vec();

    let table_ref_str = &table_aliases[0].ref_str;
    let table_ref_str_source = table_aliases[0].segment.clone();
    let mut seen_ref_types = HashSet::new();

    for reference in references.clone() {
        let mut this_ref_type = reference.qualification();
        // Placeholder parameters render as bare identifiers, but are not real
        // column references and must not affect consistency checks.
        if this_ref_type == "unqualified" && reference.0.is_templated() {
            continue;
        }
        // A bare reference to the table's own alias/name denotes the whole
        // row, not a column. It must not be qualified or affect consistency.
        if this_ref_type == "unqualified"
            && reference.0.raw_normalized().as_str() == table_ref_str.as_str()
        {
            continue;
        }
        if this_ref_type == "qualified"
            && is_struct_dialect
            && &reference
                .iter_raw_references()
                .into_iter()
                .next()
                .unwrap()
                .part
                != table_ref_str
        {
            this_ref_type = "unqualified";
        }

        let lint_res = validate_one_reference(
            tables,
            dialect_name,
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

        if let Some(fix_inconsistent_to) = fix_inconsistent_to
            .as_ref()
            .filter(|_| single_table_references == "consistent")
        {
            let results = check_references(
                tables,
                dialect_name,
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
    tables: &Tables,
    dialect_name: DialectKind,
    single_table_references: &str,
    ref_: ObjectReferenceSegment,
    this_ref_type: &str,
    standalone_aliases: &[SmolStr],
    table_ref_str: &str,
    _table_ref_str_source: Option<ErasedSegment>,
    col_alias_names: &[SmolStr],
    seen_ref_types: &HashSet<&str>,
    fixable: bool,
) -> Option<LintResult> {
    if !ref_.is_qualified() && ref_.0.is_type(SyntaxKind::WildcardIdentifier) {
        return None;
    }

    if ref_.0.is_templated() {
        return None;
    }

    if standalone_aliases.contains(ref_.0.raw()) {
        return None;
    }

    if ref_.is_qualified() {
        if ref_
            .extract_possible_references(ObjectReferenceLevel::Table, dialect_name)
            .into_iter()
            .any(|part| {
                standalone_aliases
                    .iter()
                    .any(|alias| alias.as_str() == part.part)
            })
        {
            return None;
        }

        // For multi-part references such as `item.taskId.oid`, the lambda
        // parameter is the leading (schema-level) part rather than the table-level part.
        if ref_.iter_raw_references().first().is_some_and(|part| {
            standalone_aliases
                .iter()
                .any(|alias| alias.as_str() == part.part)
        }) {
            return None;
        }
    }

    if table_ref_str.is_empty() {
        return None;
    }

    if col_alias_names.contains(ref_.0.raw()) {
        return None;
    }

    if single_table_references == "consistent" {
        return if !seen_ref_types.is_empty() && !seen_ref_types.contains(this_ref_type) {
            LintResult::new(
                ref_.clone().0.into(),
                Vec::new(),
                format!(
                    "{} reference '{}' found in single table select which is inconsistent with \
                     previous references.",
                    capitalize(this_ref_type),
                    ref_.0.raw()
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
        // If unqualified and not fixable, there is no error.
        if !fixable {
            return None;
        }

        let fixes = ref_
            .0
            .segments()
            .iter()
            .take(2)
            .cloned()
            .map(LintFix::delete)
            .collect::<Vec<_>>();

        return LintResult::new(
            ref_.0.clone().into(),
            fixes,
            format!(
                "{} reference '{}' found in single table select.",
                capitalize(this_ref_type),
                ref_.0.raw()
            )
            .into(),
            None,
        )
        .into();
    }

    let ref_ = ref_.0.clone();
    let fixes = if fixable {
        vec![LintFix::create_before(
            if !ref_.segments().is_empty() {
                ref_.segments()[0].clone()
            } else {
                ref_.clone()
            },
            vec![
                SegmentBuilder::token(tables.next_id(), table_ref_str, SyntaxKind::NakedIdentifier)
                    .finish(),
                SegmentBuilder::symbol(tables.next_id(), "."),
            ],
        )]
    } else {
        Vec::new()
    };

    LintResult::new(
        ref_.clone().into(),
        fixes,
        format!(
            "{} reference '{}' found in single table select.",
            capitalize(this_ref_type),
            ref_.raw()
        )
        .into(),
        None,
    )
    .into()
}

impl Rule for RuleRF03 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleRF03 {
            single_table_references: config
                .get("single_table_references")
                .and_then(|it| it.as_string().map(ToString::to_string)),
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "references.consistent"
    }

    fn description(&self) -> &'static str {
        "Column references should be qualified consistently in single table statements."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, only the reference to `b` is qualified.

```sql
SELECT
    a,
    foo.b
FROM foo
```

**Best practice**

Either all column references should be qualified, or all unqualified.

```sql
SELECT
    a,
    b
FROM foo

-- Also good

SELECT
    foo.a,
    foo.b
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::References]
    }

    fn force_enable(&self) -> bool {
        self.force_enable
    }

    fn dialect_skip(&self) -> &'static [DialectKind] {
        &[
            DialectKind::Athena,
            DialectKind::Bigquery,
            DialectKind::Hive,
            DialectKind::Redshift,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let single_table_references =
            self.single_table_references.as_deref().unwrap_or_else(|| {
                context.config.raw["rules"]["single_table_references"]
                    .as_string()
                    .unwrap()
            });

        let query: Query<'_> = Query::from_segment(&context.segment, context.dialect, None);
        let mut visited: HashSet<ErasedSegment> = HashSet::new();
        let is_struct_dialect = self.dialect_skip().contains(&context.dialect.name);

        Self::visit_queries(
            context.tables,
            context.dialect.name,
            single_table_references,
            is_struct_dialect,
            query,
            &mut visited,
        )
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::SelectStatement,
                    SyntaxKind::SetExpression,
                    SyntaxKind::WithCompoundStatement,
                ])
            },
        )
        .disallow_recurse()
        .into()
    }
}
