use std::ops::{Index, IndexMut};

use hashbrown::{HashMap, HashSet};
use itertools::{Itertools, enumerate};
use smol_str::{SmolStr, StrExt, ToSmolStr, format_smolstr};
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::linter::compute_anchor_edit_info;
use sqruff_lib_core::parser::segments::object_reference::ObjectReferenceLevel;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::analysis::query::{Query, Selectable};
use sqruff_lib_core::utils::analysis::select::get_select_statement_info;
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

const SELECT_TYPES: SyntaxSet = SyntaxSet::new(&[
    SyntaxKind::WithCompoundStatement,
    SyntaxKind::SetExpression,
    SyntaxKind::SelectStatement,
]);

fn config_mapping(key: &str) -> SyntaxSet {
    match key {
        "join" => SyntaxSet::single(SyntaxKind::JoinClause),
        "from" => SyntaxSet::single(SyntaxKind::FromExpressionElement),
        "both" => SyntaxSet::new(&[SyntaxKind::JoinClause, SyntaxKind::FromExpressionElement]),
        _ => unreachable!("Invalid value for 'forbid_subquery_in': {key}"),
    }
}

#[allow(dead_code)]
struct NestedSubQuerySummary<'a> {
    query: Query<'a>,
    selectable: Selectable<'a>,
    table_alias: AliasInfo,
    select_source_names: HashSet<SmolStr>,
}

struct LintQueryResult {
    lint_result: LintResult,
    from_expression: ErasedSegment,
    alias_name: SmolStr,
    subquery_parent: ErasedSegment,
    cte_source: Option<ErasedSegment>,
    is_fixable: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuleST05 {
    forbid_subquery_in: String,
}

impl Rule for RuleST05 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST05 {
            forbid_subquery_in: config["forbid_subquery_in"].as_string().unwrap().into(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "structure.subquery"
    }

    fn description(&self) -> &'static str {
        "Join/From clauses should not contain subqueries. Use CTEs instead."
    }

    fn long_description(&self) -> &'static str {
        r"
## Anti-pattern

Join is a sub query in a `FROM` clause. This can make the query harder to read and maintain.

```sql
select
    a.x, a.y, b.z
from a
join (
    select x, z from b
) using(x)
```

## Best practice

Use a Common Table Expression (CTE) to define the subquery and then join it to the main query.

```sql
with c as (
    select x, z from b
)
select
    a.x, a.y, c.z
from a
join c using(x)
```
"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let functional_context = FunctionalContext::new(context);
        let segment = functional_context.segment();
        let parent_stack = functional_context.parent_stack();

        let is_select =
            segment.all_match(|it: &ErasedSegment| SELECT_TYPES.contains(it.get_type()));
        let is_select_child =
            parent_stack.any_match(|it: &ErasedSegment| SELECT_TYPES.contains(it.get_type()));

        if !is_select || is_select_child {
            return Vec::new();
        }

        let query: Query<'_> = Query::from_segment(&context.segment, context.dialect, None);
        let mut ctes = CTEBuilder::default();

        for cte in query.inner.borrow().ctes.values() {
            ctes.insert_cte(cte.inner.borrow().cte_definition_segment.clone().unwrap());
        }

        let is_with =
            segment.all_match(|it: &ErasedSegment| it.is_type(SyntaxKind::WithCompoundStatement));
        let is_recursive = is_with
            && !segment
                .children_where(|it: &ErasedSegment| it.is_keyword("recursive"))
                .is_empty();

        let case_preference = get_case_preference(&segment);
        let output_select = if is_with {
            segment.children_where(|it: &ErasedSegment| {
                matches!(
                    it.get_type(),
                    SyntaxKind::SetExpression | SyntaxKind::SelectStatement
                )
            })
        } else {
            segment.clone()
        };
        let bracketed_ctas = parent_stack
            .base
            .iter()
            .rev()
            .take(2)
            .map(|it| it.get_type())
            .eq([SyntaxKind::CreateTableStatement, SyntaxKind::Bracketed]);

        let clone_map = SegmentCloneMap::new(segment.first().unwrap().deep_clone());

        let mut results_list = self.lint_query(
            context.tables,
            context.dialect,
            query,
            &mut ctes,
            case_preference,
            &clone_map,
        );

        let mut local_fixes = Vec::new();

        for result in &results_list {
            if !result.is_fixable {
                continue;
            }

            let this_seg_clone = clone_map[&result.from_expression].clone();
            let new_table_ref =
                create_table_ref(context.tables, &result.alias_name, context.dialect);

            local_fixes.push(LintFix::replace(
                this_seg_clone.clone(),
                vec![
                    SegmentBuilder::node(
                        context.tables.next_id(),
                        this_seg_clone.get_type(),
                        context.dialect.name,
                        vec![new_table_ref],
                    )
                    .finish(),
                ],
                None,
            ));
        }

        if bracketed_ctas || is_recursive || local_fixes.is_empty() {
            return results_list
                .into_iter()
                .map(|result| result.lint_result)
                .collect();
        }

        let mut fixes = HashMap::default();
        compute_anchor_edit_info(&mut fixes, local_fixes);
        let (new_root, _, _) = clone_map.root.apply_fixes(&mut fixes);

        let clone_map = SegmentCloneMap::new(new_root.clone());
        for result in &results_list {
            if result.is_fixable {
                ctes.replace_with_clone(result.subquery_parent.clone(), &clone_map);
            }
        }
        for result in &results_list {
            let Some(cte_source) = &result.cte_source else {
                continue;
            };

            let cte_source_clone = cte_source.deep_clone();
            let cte_clone_map = SegmentCloneMap::new(cte_source_clone.clone());
            let mut cte_fixes = Vec::new();

            for nested_result in &results_list {
                if !nested_result.is_fixable
                    || cte_clone_map.get(&nested_result.from_expression).is_none()
                {
                    continue;
                }

                let from_expression_clone = cte_clone_map[&nested_result.from_expression].clone();
                let new_table_ref =
                    create_table_ref(context.tables, &nested_result.alias_name, context.dialect);

                cte_fixes.push(LintFix::replace(
                    from_expression_clone.clone(),
                    vec![
                        SegmentBuilder::node(
                            context.tables.next_id(),
                            from_expression_clone.get_type(),
                            context.dialect.name,
                            vec![new_table_ref],
                        )
                        .finish(),
                    ],
                    None,
                ));
            }

            let cte_source = if cte_fixes.is_empty() {
                cte_source_clone
            } else {
                let mut fixes = HashMap::default();
                compute_anchor_edit_info(&mut fixes, cte_fixes);
                cte_source_clone.apply_fixes(&mut fixes).0
            };

            let new_cte = create_cte_seg(
                context.tables,
                result.alias_name.clone(),
                cte_source,
                case_preference,
                context.dialect,
            );
            ctes.replace_cte(&result.alias_name, new_cte);
        }

        // If there's no SELECT statement (e.g., WITH ... INSERT/UPDATE/DELETE),
        // we can't safely create fixes, so return lint results without fixes.
        if output_select.is_empty() {
            return results_list
                .into_iter()
                .map(|result| result.lint_result)
                .collect();
        }

        let mut output_select_clone = clone_map[&output_select[0]].clone();

        for result in &mut results_list {
            if !result.is_fixable {
                continue;
            }

            let mut fixes = ctes.ensure_space_after_from(
                context.tables,
                output_select[0].clone(),
                &mut output_select_clone,
                result.subquery_parent.clone(),
            );

            let new_select = ctes.compose_select(
                context.tables,
                context.dialect.name,
                output_select_clone.clone(),
                case_preference,
            );

            result.lint_result.fixes = vec![LintFix::replace(
                segment.first().unwrap().clone(),
                vec![new_select],
                None,
            )];

            result.lint_result.fixes.append(&mut fixes);
        }

        results_list
            .into_iter()
            .map(|result| result.lint_result)
            .collect()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(SELECT_TYPES).into()
    }
}

impl RuleST05 {
    fn lint_query<'a>(
        &self,
        tables: &Tables,
        dialect: &'a Dialect,
        query: Query<'a>,
        ctes: &mut CTEBuilder,
        case_preference: Case,
        segment_clone_map: &SegmentCloneMap,
    ) -> Vec<LintQueryResult> {
        let mut acc = Vec::new();

        for nsq in self.nested_subqueries(query, dialect) {
            let (alias_name, _) = ctes.create_cte_alias(Some(&nsq.table_alias));
            let anchor = nsq
                .table_alias
                .from_expression_element
                .segments()
                .first()
                .cloned()
                .unwrap();

            let mut is_fixable = !ctes.list_used_names().contains(&alias_name);
            let bracket_anchor = if anchor.is_type(SyntaxKind::TableExpression) {
                let Some(bracket_anchor) =
                    anchor.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
                else {
                    continue;
                };

                bracket_anchor
            } else {
                anchor.clone()
            };

            if !bracket_anchor.is_type(SyntaxKind::Bracketed)
                || bracket_anchor
                    .child(const { &SyntaxSet::single(SyntaxKind::TableExpression) })
                    .is_some()
            {
                is_fixable = false;
            }

            if is_fixable {
                let new_cte = create_cte_seg(
                    tables,
                    alias_name.clone(),
                    segment_clone_map[&bracket_anchor].clone(),
                    case_preference,
                    dialect,
                );

                ctes.insert_cte(new_cte);
            }

            let anchor = anchor.recursive_crawl(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::Keyword,
                        SyntaxKind::Symbol,
                        SyntaxKind::StartBracket,
                        SyntaxKind::EndBracket,
                    ])
                },
                true,
                &SyntaxSet::EMPTY,
                true,
            )[0]
            .clone();

            let res = LintResult::new(
                anchor.into(),
                Vec::new(),
                format!(
                    "{} clauses should not contain subqueries. Use CTEs instead",
                    nsq.selectable.selectable.get_type().as_str()
                )
                .into(),
                None,
            );

            acc.push(LintQueryResult {
                lint_result: res,
                from_expression: nsq.table_alias.from_expression_element,
                alias_name: alias_name.clone(),
                subquery_parent: nsq.selectable.selectable,
                cte_source: is_fixable.then_some(bracket_anchor),
                is_fixable,
            });
        }

        acc
    }

    fn nested_subqueries<'a>(
        &self,
        query: Query<'a>,
        dialect: &'a Dialect,
    ) -> Vec<NestedSubQuerySummary<'a>> {
        let mut acc = Vec::new();

        let parent_types = config_mapping(&self.forbid_subquery_in);
        let mut queries = vec![query.clone()];
        queries.extend(query.inner.borrow().ctes.values().cloned());

        for (i, q) in enumerate(queries) {
            for selectable in &q.inner.borrow().selectables {
                let Some(select_info) = selectable.select_info() else {
                    continue;
                };

                let mut select_source_names = HashSet::new();
                for table_alias in select_info.table_aliases {
                    if !table_alias.ref_str.is_empty() {
                        select_source_names.insert(table_alias.ref_str.clone());
                    }

                    if let Some(object_reference) = &table_alias.object_reference {
                        select_source_names.insert(object_reference.raw().to_smolstr());
                    }

                    let Some(query) =
                        Query::from_root(&table_alias.from_expression_element, dialect)
                    else {
                        continue;
                    };

                    let path_to = selectable
                        .selectable
                        .path_to(&table_alias.from_expression_element);

                    if !(parent_types.contains(table_alias.from_expression_element.get_type())
                        || path_to
                            .iter()
                            .any(|ps| parent_types.contains(ps.segment.get_type())))
                    {
                        continue;
                    }

                    if is_correlated_subquery(
                        Segments::new(
                            query
                                .inner
                                .borrow()
                                .selectables
                                .first()
                                .unwrap()
                                .selectable
                                .clone(),
                            None,
                        ),
                        &select_source_names,
                        dialect,
                    ) {
                        continue;
                    }

                    acc.push(NestedSubQuerySummary {
                        query: q.clone(),
                        selectable: selectable.clone(),
                        table_alias: table_alias.clone(),
                        select_source_names: select_source_names.clone(),
                    });

                    if i > 0 {
                        acc.append(&mut self.nested_subqueries(query.clone(), dialect));
                    }
                }
            }
        }

        acc
    }
}

fn get_first_select_statement_descendant(segment: &ErasedSegment) -> Option<ErasedSegment> {
    segment
        .recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            false,
            &SyntaxSet::EMPTY,
            true,
        )
        .into_iter()
        .next()
}

fn is_correlated_subquery(
    nested_select: Segments,
    select_source_names: &HashSet<SmolStr>,
    dialect: &Dialect,
) -> bool {
    let Some(select_statement) =
        get_first_select_statement_descendant(nested_select.first().unwrap())
    else {
        return false;
    };

    let nested_select_info = get_select_statement_info(&select_statement, dialect.into(), true);
    if let Some(nested_select_info) = nested_select_info {
        for r in nested_select_info.reference_buffer {
            for tr in r.extract_possible_references(ObjectReferenceLevel::Table, dialect.name) {
                if select_source_names.contains(&tr.part.to_smolstr()) {
                    return true;
                }
            }
        }
    }

    false
}

#[derive(Default)]
struct CTEBuilder {
    ctes: Vec<ErasedSegment>,
    name_idx: usize,
}

impl CTEBuilder {
    fn get_cte_segments(&self, tables: &Tables) -> Vec<ErasedSegment> {
        let mut cte_segments = Vec::new();
        let mut ctes = self.ctes.iter().peekable();

        while let Some(cte) = ctes.next() {
            cte_segments.push(cte.clone());

            if ctes.peek().is_some() {
                cte_segments.extend([
                    SegmentBuilder::comma(tables.next_id()),
                    SegmentBuilder::newline(tables.next_id(), "\n"),
                ]);
            }
        }

        cte_segments
    }

    fn compose_select(
        &self,
        tables: &Tables,
        dialect: DialectKind,
        output_select_clone: ErasedSegment,
        case_preference: Case,
    ) -> ErasedSegment {
        let mut segments = vec![
            segmentify(tables, "WITH", case_preference),
            SegmentBuilder::whitespace(tables.next_id(), " "),
        ];
        segments.extend(self.get_cte_segments(tables));
        segments.push(SegmentBuilder::newline(tables.next_id(), "\n"));
        segments.push(output_select_clone);

        SegmentBuilder::node(
            tables.next_id(),
            SyntaxKind::WithCompoundStatement,
            dialect,
            segments,
        )
        .finish()
    }
}

impl CTEBuilder {
    fn ensure_space_after_from(
        &self,
        tables: &Tables,
        output_select: ErasedSegment,
        output_select_clone: &mut ErasedSegment,
        subquery_parent: ErasedSegment,
    ) -> Vec<LintFix> {
        let mut fixes = Vec::new();

        if subquery_parent.is(&output_select) {
            let (missing_space_after_from, _from_clause, _from_clause_children, from_segment) =
                Self::missing_space_after_from(output_select_clone.clone());

            if missing_space_after_from {
                let mut anchor_fixes = HashMap::default();
                compute_anchor_edit_info(
                    &mut anchor_fixes,
                    vec![LintFix::create_after(
                        from_segment.unwrap().base[0].clone(),
                        vec![SegmentBuilder::whitespace(tables.next_id(), " ")],
                        None,
                    )],
                );
                *output_select_clone = output_select_clone.apply_fixes(&mut anchor_fixes).0;
            }
        } else {
            let (missing_space_after_from, _from_clause, _from_clause_children, from_segment) =
                Self::missing_space_after_from(subquery_parent);

            if missing_space_after_from {
                fixes.push(LintFix::create_after(
                    from_segment.unwrap().base[0].clone(),
                    vec![SegmentBuilder::whitespace(tables.next_id(), " ")],
                    None,
                ))
            }
        }

        fixes
    }

    fn missing_space_after_from(
        segment: ErasedSegment,
    ) -> (
        bool,
        Option<ErasedSegment>,
        Option<ErasedSegment>,
        Option<Segments>,
    ) {
        let mut missing_space_after_from = false;
        let from_clause_children = None;
        let mut from_segment = None;
        let from_clause = segment.child(const { &SyntaxSet::single(SyntaxKind::FromClause) });

        if let Some(from_clause) = &from_clause {
            let from_clause_children = Segments::from_vec(from_clause.segments().to_vec(), None);
            from_segment = from_clause_children
                .find_first_where(|it: &ErasedSegment| it.is_keyword("FROM"))
                .into();
            if !from_segment.as_ref().unwrap().is_empty()
                && from_clause_children
                    .after(&from_segment.as_ref().unwrap().base[0])
                    .take_while(|it| it.is_whitespace())
                    .is_empty()
            {
                missing_space_after_from = true;
            }
        }

        (
            missing_space_after_from,
            from_clause,
            from_clause_children,
            from_segment,
        )
    }
}

impl CTEBuilder {
    pub(crate) fn replace_with_clone(
        &mut self,
        segment: ErasedSegment,
        clone_map: &SegmentCloneMap,
    ) {
        for (idx, cte) in enumerate(&self.ctes) {
            if cte
                .recursive_crawl_all(false)
                .into_iter()
                .any(|seg| segment.is(&seg))
            {
                if let Some(cte_clone) = clone_map.get(&self.ctes[idx]) {
                    self.ctes[idx] = cte_clone.clone();
                }
                return;
            }
        }
    }
}

impl CTEBuilder {
    fn cte_name(cte: &ErasedSegment) -> SmolStr {
        let id_seg = cte
            .child(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::Identifier,
                        SyntaxKind::NakedIdentifier,
                        SyntaxKind::QuotedIdentifier,
                    ])
                },
            )
            .unwrap();

        if id_seg.is_type(SyntaxKind::QuotedIdentifier) {
            let raw = id_seg.raw();
            raw[1..raw.len() - 1].to_smolstr()
        } else {
            id_seg.raw().to_smolstr()
        }
    }

    fn list_used_names(&self) -> Vec<SmolStr> {
        self.ctes.iter().map(Self::cte_name).collect()
    }

    fn replace_cte(&mut self, cte_name: &str, new_cte: ErasedSegment) {
        if let Some(idx) = self
            .ctes
            .iter()
            .position(|cte| Self::cte_name(cte) == cte_name)
        {
            self.ctes[idx] = new_cte;
        }
    }

    fn create_cte_alias(&mut self, alias: Option<&AliasInfo>) -> (SmolStr, bool) {
        if let Some(alias) = alias.filter(|alias| alias.aliased && !alias.ref_str.is_empty()) {
            return (alias.ref_str.clone(), false);
        }

        self.name_idx += 1;
        let name = format_smolstr!("prep_{}", self.name_idx);
        if self.list_used_names().iter().contains(&name) {
            return self.create_cte_alias(None);
        }

        (name, true)
    }

    fn insert_cte(&mut self, cte: ErasedSegment) {
        let inbound_subquery = Segments::new(cte.clone(), None)
            .children_all()
            .find_first_where(|it: &ErasedSegment| it.get_position_marker().is_some());
        let insert_position = self
            .ctes
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, it)| {
                is_child(
                    Segments::new(
                        Segments::new(it.clone(), None)
                            .children_all()
                            .last()
                            .cloned()
                            .unwrap(),
                        None,
                    ),
                    inbound_subquery.clone(),
                )
            })
            .map(|(i, _)| i)
            .next()
            .unwrap_or(self.ctes.len());

        self.ctes.insert(insert_position, cte);
    }
}

fn is_child(maybe_parent: Segments, maybe_child: Segments) -> bool {
    let child_markers = maybe_child[0].get_position_marker().unwrap();
    let parent_pos = maybe_parent[0].get_position_marker().unwrap();

    if child_markers < &parent_pos.start_point_marker() {
        return false;
    }

    if child_markers > &parent_pos.end_point_marker() {
        return false;
    }

    true
}

#[derive(Eq, PartialEq, Copy, Clone)]
enum Case {
    Lower,
    Upper,
}

fn get_case_preference(root_select: &Segments) -> Case {
    let root_segment = root_select.first().expect("Root SELECT not found");
    let first_keyword = root_segment.recursive_crawl(
        const { &SyntaxSet::single(SyntaxKind::Keyword) },
        false,
        &SyntaxSet::EMPTY,
        true,
    )[0]
    .clone();

    if first_keyword.raw().chars().all(char::is_lowercase) {
        Case::Lower
    } else {
        Case::Upper
    }
}

fn segmentify(tables: &Tables, input_el: &str, casing: Case) -> ErasedSegment {
    let mut input_el = input_el.to_lowercase_smolstr();
    if casing == Case::Upper {
        input_el = input_el.to_uppercase_smolstr();
    }
    SegmentBuilder::keyword(tables.next_id(), &input_el)
}

fn create_cte_seg(
    tables: &Tables,
    alias_name: SmolStr,
    subquery: ErasedSegment,
    case_preference: Case,
    dialect: &Dialect,
) -> ErasedSegment {
    SegmentBuilder::node(
        tables.next_id(),
        SyntaxKind::CommonTableExpression,
        dialect.name,
        vec![
            SegmentBuilder::token(tables.next_id(), &alias_name, SyntaxKind::NakedIdentifier)
                .finish(),
            SegmentBuilder::whitespace(tables.next_id(), " "),
            segmentify(tables, "AS", case_preference),
            SegmentBuilder::whitespace(tables.next_id(), " "),
            subquery,
        ],
    )
    .finish()
}

fn create_table_ref(tables: &Tables, table_name: &str, dialect: &Dialect) -> ErasedSegment {
    SegmentBuilder::node(
        tables.next_id(),
        SyntaxKind::TableExpression,
        dialect.name,
        vec![
            SegmentBuilder::node(
                tables.next_id(),
                SyntaxKind::TableReference,
                dialect.name,
                vec![
                    SegmentBuilder::token(
                        tables.next_id(),
                        table_name,
                        SyntaxKind::NakedIdentifier,
                    )
                    .finish(),
                ],
            )
            .finish(),
        ],
    )
    .finish()
}

pub(crate) struct SegmentCloneMap {
    root: ErasedSegment,
    segment_map: HashMap<u32, ErasedSegment>,
}

impl Index<&ErasedSegment> for SegmentCloneMap {
    type Output = ErasedSegment;

    fn index(&self, index: &ErasedSegment) -> &Self::Output {
        &self.segment_map[&index.id()]
    }
}

impl IndexMut<&ErasedSegment> for SegmentCloneMap {
    fn index_mut(&mut self, index: &ErasedSegment) -> &mut Self::Output {
        self.segment_map.get_mut(&index.id()).unwrap()
    }
}

impl SegmentCloneMap {
    fn get(&self, old_segment: &ErasedSegment) -> Option<&ErasedSegment> {
        self.segment_map.get(&old_segment.id())
    }

    fn new(segment_copy: ErasedSegment) -> Self {
        let mut segment_map = HashMap::new();

        for segment in segment_copy.recursive_crawl_all(false) {
            segment_map.insert(segment.id(), segment.clone());
        }

        Self {
            root: segment_copy,
            segment_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    fn st05_linter(dialect: &str) -> Linter {
        let config = FluffConfig::from_source(
            &format!(
                r#"
[sqruff]
dialect = {dialect}
rules = ST05

[sqruff:rules:structure.subquery]
forbid_subquery_in = both
"#
            ),
            None,
        );

        Linter::new(config, None, None, false).unwrap()
    }

    fn assert_fix(dialect: &str, source: &str, expected: &str) {
        let mut linter = st05_linter(dialect);
        let actual = linter
            .lint_string_wrapped(source, true)
            .unwrap()
            .fix_string();

        pretty_assertions::assert_eq!(actual, expected);
    }

    #[test]
    fn st05_fixes_double_nested_subquery_without_panicking() {
        let source = r#"WITH q AS (
    SELECT
        t1.a
    FROM
        table_1 AS t1
    INNER JOIN
        table_2 AS t2 USING (a)
    LEFT JOIN (
        SELECT DISTINCT a FROM table_3
            WHERE c = 'v1'
    ) AS dns USING (a)
    LEFT JOIN (
        SELECT DISTINCT a FROM table_5
        LEFT JOIN (
            SELECT DISTINCT
                a,
                b
            FROM table_6
            WHERE c < 5
        ) AS t4
            USING (a)
        WHERE table_5.b = 'v2'
    ) AS dcod USING (a)
)
SELECT
    a
FROM
    q;
"#;
        let expected = r#"WITH dns AS (
        SELECT DISTINCT a FROM table_3
            WHERE c = 'v1'
    ),
t4 AS (
            SELECT DISTINCT
                a,
                b
            FROM table_6
            WHERE c < 5
        ),
dcod AS (
        SELECT DISTINCT a FROM table_5
        LEFT JOIN t4
            USING (a)
        WHERE table_5.b = 'v2'
    ),
q AS (
    SELECT
        t1.a
    FROM
        table_1 AS t1
    INNER JOIN
        table_2 AS t2 USING (a)
    LEFT JOIN dns USING (a)
    LEFT JOIN dcod USING (a)
)
SELECT
    a
FROM
    q;
"#;

        assert_fix("ansi", source, expected);
    }

    #[test]
    fn st05_fixes_order_4782_without_panicking() {
        let source = r#"WITH
cte_1 AS (
    SELECT
        subquery_a.field_a,
        subquery_a.field_b
    FROM (
        SELECT
            subquery_b.field_a,
            alias_a.field_d,
            alias_a.field_b,
            alias_b.field_c
        FROM table_b AS alias_a
        INNER JOIN
            (SELECT * FROM table_a) AS subquery_b
            ON subquery_b.field_a >= alias_a.field_d
        LEFT OUTER JOIN table_b AS alias_b ON alias_a.field_b = alias_b.field_c
    ) AS subquery_a
),

cte_2 AS (
    SELECT *
    FROM table_c
    WHERE field_a > 0
    ORDER BY field_b DESC
),

join_ctes AS (
    SELECT * FROM cte_1 LEFT OUTER JOIN cte_2 ON cte_1.field_a = cte_2.field_a
)

SELECT *
FROM join_ctes;
"#;
        let expected = r#"WITH subquery_b AS (SELECT * FROM table_a),
subquery_a AS (
        SELECT
            subquery_b.field_a,
            alias_a.field_d,
            alias_a.field_b,
            alias_b.field_c
        FROM table_b AS alias_a
        INNER JOIN
            subquery_b
            ON subquery_b.field_a >= alias_a.field_d
        LEFT OUTER JOIN table_b AS alias_b ON alias_a.field_b = alias_b.field_c
    ),
cte_1 AS (
    SELECT
        subquery_a.field_a,
        subquery_a.field_b
    FROM subquery_a
),
cte_2 AS (
    SELECT *
    FROM table_c
    WHERE field_a > 0
    ORDER BY field_b DESC
),
join_ctes AS (
    SELECT * FROM cte_1 LEFT OUTER JOIN cte_2 ON cte_1.field_a = cte_2.field_a
)
SELECT *
FROM join_ctes;
"#;

        assert_fix("ansi", source, expected);
    }

    #[test]
    fn st05_fixes_same_named_nested_subqueries_across_ctes() {
        let source = r#"with purchases_in_the_last_year as (
    select
        customer_id
        , arrayagg(distinct attr) within group (order by attr asc) as attrlist
    from (
        select
            o.customer_id
            , p.attr
        from
            order_line_item as o
        inner join product as p
            on o.product_id = p.product_id
            and o.time_placed >= dateadd(year, -1, current_date())
    ) group by customer_id
)

, purchases_in_the_last_three_years as (
    select
        customer_id
        , arrayagg(distinct attr) within group (order by attr asc) as attrlist
    from (
        select
            o.customer_id
            , p.attr
        from
            order_line_item as o
        inner join product as p
            on o.product_id = p.product_id
            and o.time_placed >= dateadd(year, -3, current_date())
    ) group by customer_id
)


select distinct
    c.customer_id
    , ly.attrlist as attrlist_last_year
    , l3y.attrlist as attrlist_last_three_years
from
    customers as c
left outer join
    purchases_in_the_last_year as ly
    on c.customer_id = ly.customer_id
left outer join
    purchases_in_the_last_three_years as l3y
    on c.customer_id = l3y.customer_id
;
"#;
        let expected = r#"with prep_1 as (
        select
            o.customer_id
            , p.attr
        from
            order_line_item as o
        inner join product as p
            on o.product_id = p.product_id
            and o.time_placed >= dateadd(year, -1, current_date())
    ),
purchases_in_the_last_year as (
    select
        customer_id
        , arrayagg(distinct attr) within group (order by attr asc) as attrlist
    from prep_1 group by customer_id
),
prep_2 as (
        select
            o.customer_id
            , p.attr
        from
            order_line_item as o
        inner join product as p
            on o.product_id = p.product_id
            and o.time_placed >= dateadd(year, -3, current_date())
    ),
purchases_in_the_last_three_years as (
    select
        customer_id
        , arrayagg(distinct attr) within group (order by attr asc) as attrlist
    from prep_2 group by customer_id
)
select distinct
    c.customer_id
    , ly.attrlist as attrlist_last_year
    , l3y.attrlist as attrlist_last_three_years
from
    customers as c
left outer join
    purchases_in_the_last_year as ly
    on c.customer_id = ly.customer_id
left outer join
    purchases_in_the_last_three_years as l3y
    on c.customer_id = l3y.customer_id
;
"#;

        assert_fix("snowflake", source, expected);
    }

    #[test]
    fn st05_partially_fixes_duplicate_aliases_in_order_5265_case() {
        let source = r#"WITH
cte1 AS (
    SELECT COUNT(*) AS qty
    FROM some_table AS st
    LEFT JOIN (
        SELECT 'first' AS id
    ) AS oops
    ON st.id = oops.id
),
cte2 AS (
    SELECT COUNT(*) AS other_qty
    FROM other_table AS sot
    LEFT JOIN (
        SELECT 'middle' AS id
    ) AS another
    ON sot.id = another.id
    LEFT JOIN (
        SELECT 'last' AS id
    ) AS oops
    ON sot.id = oops.id
)
SELECT CURRENT_DATE();
"#;
        let expected = r#"WITH oops AS (
        SELECT 'first' AS id
    ),
cte1 AS (
    SELECT COUNT(*) AS qty
    FROM some_table AS st
    LEFT JOIN oops
    ON st.id = oops.id
),
another AS (
        SELECT 'middle' AS id
    ),
cte2 AS (
    SELECT COUNT(*) AS other_qty
    FROM other_table AS sot
    LEFT JOIN another
    ON sot.id = another.id
    LEFT JOIN (
        SELECT 'last' AS id
    ) AS oops
    ON sot.id = oops.id
)
SELECT CURRENT_DATE();
"#;

        assert_fix("ansi", source, expected);
    }

    #[test]
    fn st05_inserts_space_after_from_when_rewriting_root_subquery() {
        let source = r#"CREATE TABLE t
AS
SELECT
    col1
FROM(
    SELECT 'x' AS col1
) x
"#;
        let expected = r#"CREATE TABLE t
AS
WITH x AS (
    SELECT 'x' AS col1
)
SELECT
    col1
FROM x
"#;

        assert_fix("ansi", source, expected);
    }

    #[test]
    fn st05_fixes_set_subquery_in_second_query() {
        let source = r#"SELECT 1 AS value_name
UNION
SELECT value
FROM (SELECT 2 AS value_name);
"#;
        let expected = r#"WITH prep_1 AS (SELECT 2 AS value_name)
SELECT 1 AS value_name
UNION
SELECT value
FROM prep_1;
"#;

        assert_fix("ansi", source, expected);
    }

    #[test]
    fn st05_fixes_multiple_set_subqueries_in_second_query() {
        let source = r#"SELECT 1 AS value_name
UNION
SELECT value
FROM (SELECT 2 AS value_name)
CROSS JOIN (SELECT 1 as v2);
"#;
        let expected = r#"WITH prep_1 AS (SELECT 2 AS value_name),
prep_2 AS (SELECT 1 as v2)
SELECT 1 AS value_name
UNION
SELECT value
FROM prep_1
CROSS JOIN prep_2;
"#;

        assert_fix("ansi", source, expected);
    }
}
