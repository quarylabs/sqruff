use std::iter::zip;
use std::ops::{Index, IndexMut};

use ahash::{AHashMap, AHashSet};
use itertools::{enumerate, Itertools};
use smol_str::{format_smolstr, SmolStr, StrExt, ToSmolStr};

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::segments::base::{
    CodeSegmentNewArgs, ErasedSegment, IdentifierSegment, NewlineSegment, NewlineSegmentNewArgs,
    Segment, SymbolSegment, SymbolSegmentNewArgs, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::{CTEDefinitionSegment, Node, ObjectReferenceLevel};
use crate::dialects::SyntaxKind;
use crate::helpers::ToErasedSegment;
use crate::utils::analysis::query::{Query, Selectable};
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::context::FunctionalContext;
use crate::utils::functional::segments::Segments;

const SELECT_TYPES: [&str; 3] = ["with_compound_statement", "set_expression", "select_statement"];

static CONFIG_MAPPING: phf::Map<&str, &[&str]> = phf::phf_map! {
    "join" => &["join_clause"],
    "from" => &["from_expression_element"],
    "both" => &["join_clause", "from_expression_element"]
};

struct NestedSubQuerySummary<'a> {
    query: Query<'a, ()>,
    selectable: Selectable<'a>,
    table_alias: AliasInfo,
    select_source_names: AHashSet<SmolStr>,
}

#[derive(Clone, Debug, Default)]
pub struct RuleST05 {
    forbid_subquery_in: String,
}

impl Rule for RuleST05 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST05 {
            forbid_subquery_in: config["forbid_subquery_in"].as_string().unwrap().into(),
        }
        .erased())
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "structure.subquery"
    }

    fn description(&self) -> &'static str {
        "Join/From clauses should not contain subqueries. Use CTEs instead."
    }

    fn long_description(&self) -> &'static str {
        ""
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let functional_context = FunctionalContext::new(context.clone());
        let segment = functional_context.segment();
        let parent_stack = functional_context.parent_stack();

        let is_select =
            segment.all(Some(|it: &ErasedSegment| SELECT_TYPES.contains(&it.get_type())));
        let is_select_child =
            parent_stack.any(Some(|it: &ErasedSegment| SELECT_TYPES.contains(&it.get_type())));

        if !is_select || is_select_child {
            return Vec::new();
        }

        let query: Query<'_, ()> = Query::from_segment(&context.segment, context.dialect, None);
        let mut ctes = CTEBuilder::default();
        for cte in query.inner.borrow().ctes.values() {
            ctes.insert_cte(CTEDefinitionSegment(
                cte.inner.borrow().cte_definition_segment.clone().unwrap(),
            ));
        }

        let is_with = segment.all(Some(|it: &ErasedSegment| it.is_type("with_compound_statement")));
        let is_recursive = is_with
            && !segment.children(Some(|it: &ErasedSegment| it.is_keyword("recursive"))).is_empty();

        let case_preference = get_case_preference(&segment);
        let output_select = if is_with {
            segment.children(Some(|it: &ErasedSegment| {
                matches!(it.get_type(), "set_expression" | "select_statement")
            }))
        } else {
            segment.clone()
        };

        let mut clone_map = SegmentCloneMap::new(segment.first().unwrap().clone());
        let results =
            self.lint_query(context.dialect, query, &mut ctes, case_preference, &clone_map);

        let mut lint_results = Vec::with_capacity(results.len());
        let mut is_fixable = true;

        let mut subquery_parent = None;

        for result in results {
            let (lint_result, from_expression, alias_name, subquery_parent_slot) = result;
            subquery_parent = Some(subquery_parent_slot.clone());
            let this_seg_clone = &mut clone_map[&from_expression];
            let new_table_ref = create_table_ref(&alias_name, context.dialect);

            this_seg_clone.get_mut().set_segments(vec![new_table_ref]);
            dbg!(this_seg_clone.raw());
            ctes.replace_with_clone(subquery_parent_slot, &clone_map);

            let bracketed_ctas = parent_stack
                .base
                .iter()
                .rev()
                .take(2)
                .map(|it| it.get_type())
                .eq(["create_table_statement", "bracketed"]);

            if bracketed_ctas || is_recursive {
                is_fixable = false;
            }

            lint_results.push(lint_result);
        }

        if !is_fixable {
            return lint_results;
        }

        for result in &mut lint_results {
            let subquery_parent = subquery_parent.clone().unwrap();
            let output_select_clone = &clone_map[&output_select[0]];
            let mut fixes = ctes.ensure_space_after_from(
                output_select[0].clone(),
                output_select_clone,
                subquery_parent,
            );
            let new_select = ctes.compose_select(
                context.dialect.name,
                output_select_clone.clone(),
                case_preference,
            );

            println!("{}", output_select_clone.raw());

            std::process::exit(0);

            result.fixes =
                vec![LintFix::replace(segment.first().unwrap().clone(), vec![new_select], None)];

            result.fixes.append(&mut fixes);
        }

        lint_results
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(SELECT_TYPES.into()).into()
    }
}

impl RuleST05 {
    fn lint_query<'a>(
        &self,
        dialect: &'a Dialect,
        query: Query<'a, ()>,
        ctes: &mut CTEBuilder,
        case_preference: Case,
        segment_clone_map: &SegmentCloneMap,
    ) -> Vec<(LintResult, ErasedSegment, SmolStr, ErasedSegment)> {
        let mut acc = Vec::new();

        for nsq in self.nested_subqueries(query, dialect) {
            let (alias_name, _) = ctes.create_cte_alias(Some(&nsq.table_alias));
            let anchor =
                nsq.table_alias.from_expression_element.segments().first().cloned().unwrap();

            let new_cte = create_cte_seg(
                alias_name.clone(),
                segment_clone_map[&anchor].clone(),
                case_preference,
                dialect,
            );
            ctes.insert_cte(CTEDefinitionSegment(new_cte));

            if nsq.query.inner.borrow().selectables.len() != 1 {
                continue;
            }

            let select = nsq.query.inner.borrow().selectables[0].clone().selectable;
            let anchor =
                anchor.recursive_crawl(&["keyword", "symbol"], true, None, true)[0].clone();
            let res = LintResult::new(
                anchor.into(),
                Vec::new(),
                None,
                format!(
                    "{} clauses should not contain subqueries. Use CTEs instead",
                    select.get_type()
                )
                .into(),
                None,
            );

            acc.push((
                res,
                nsq.table_alias.from_expression_element,
                alias_name.clone(),
                nsq.query.inner.borrow().selectables[0].clone().selectable,
            ));
        }

        acc
    }

    fn nested_subqueries<'a>(
        &self,
        query: Query<'a, ()>,
        dialect: &'a Dialect,
    ) -> Vec<NestedSubQuerySummary<'a>> {
        let mut acc = Vec::new();

        let parent_types = CONFIG_MAPPING[&self.forbid_subquery_in];
        let mut queries = vec![query.clone()];
        queries.extend(query.inner.borrow().ctes.values().cloned());

        for (i, q) in enumerate(queries) {
            for selectable in &q.inner.borrow().selectables {
                let Some(select_info) = selectable.select_info() else {
                    continue;
                };

                let mut select_source_names = AHashSet::new();
                for table_alias in select_info.table_aliases {
                    if !table_alias.ref_str.is_empty() {
                        select_source_names.insert(table_alias.ref_str.clone());
                    }

                    if let Some(object_reference) = &table_alias.object_reference {
                        select_source_names.insert(object_reference.raw().to_smolstr());
                    }

                    let Some(query) =
                        Query::<()>::from_root(&table_alias.from_expression_element, dialect)
                    else {
                        continue;
                    };

                    let path_to =
                        selectable.selectable.path_to(&table_alias.from_expression_element);

                    if !(parent_types
                        .iter()
                        .any(|typ| table_alias.from_expression_element.is_type(typ))
                        || path_to
                            .iter()
                            .any(|ps| parent_types.iter().any(|typ| ps.segment.is_type(typ))))
                    {
                        continue;
                    }

                    if is_correlated_subquery(
                        Segments::new(
                            query.inner.borrow().selectables.first().unwrap().selectable.clone(),
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
    segment.recursive_crawl(&["select_statement"], false, None, true).into_iter().next()
}

fn is_correlated_subquery(
    nested_select: Segments,
    select_source_names: &AHashSet<SmolStr>,
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
                if select_source_names.contains(&tr.part) {
                    return true;
                }
            }
        }
    }

    false
}

#[derive(Default)]
struct CTEBuilder {
    ctes: Vec<CTEDefinitionSegment>,
    name_idx: usize,
}

impl CTEBuilder {
    fn get_cte_segments(&self) -> Vec<ErasedSegment> {
        let mut cte_segments = Vec::new();
        let mut ctes = self.ctes.iter().peekable();

        while let Some(cte) = ctes.next() {
            cte_segments.push(cte.0.clone());

            if ctes.peek().is_some() {
                cte_segments.extend([
                    SymbolSegment::create(",", None, SymbolSegmentNewArgs { r#type: "," }),
                    NewlineSegment::create("\n", None, NewlineSegmentNewArgs {}),
                ]);
            }
        }

        cte_segments
    }

    fn compose_select(
        &self,
        dialect: DialectKind,
        output_select_clone: ErasedSegment,
        case_preference: Case,
    ) -> ErasedSegment {
        let mut segments = vec![
            segmentify("WITH", case_preference),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs {}),
        ];
        segments.extend(self.get_cte_segments());
        segments.push(NewlineSegment::create("\n", None, NewlineSegmentNewArgs {}));
        segments.push(output_select_clone);

        Node::new(dialect, SyntaxKind::WithCompoundStatement, segments, false).to_erased_segment()
    }
}

impl CTEBuilder {
    fn ensure_space_after_from(
        &self,
        output_select: ErasedSegment,
        output_select_clone: &ErasedSegment,
        subquery_parent: ErasedSegment,
    ) -> Vec<LintFix> {
        let mut fixes = Vec::new();

        if subquery_parent.is(&output_select) {
            let (missing_space_after_from, from_clause, from_clause_children, from_segment) =
                Self::missing_space_after_from(output_select_clone.clone());

            if missing_space_after_from {
                todo!()
            }
        } else {
            todo!()
        }

        fixes
    }

    fn missing_space_after_from(
        segment: ErasedSegment,
    ) -> (bool, Option<ErasedSegment>, Option<ErasedSegment>, Option<ErasedSegment>) {
        let mut missing_space_after_from = false;
        let from_clause_children = None;
        let from_segment = None;
        let from_clause = segment.child(&["from_clause"]);

        if let Some(from_clause) = &from_clause {
            let from_clause_children = Segments::from_vec(from_clause.segments().to_vec(), None);
            let from_segment =
                from_clause_children.find_first(Some(|it: &ErasedSegment| it.is_keyword("FROM")));
            if !from_segment.is_empty()
                && from_clause_children
                    .select(None, Some(|it| it.is_whitespace()), Some(&from_segment[0]), None)
                    .is_empty()
            {
                missing_space_after_from = true;
            }
        }

        (missing_space_after_from, from_clause, from_clause_children, from_segment)
    }
}

impl CTEBuilder {
    pub(crate) fn replace_with_clone(&self, p0: ErasedSegment, p1: &SegmentCloneMap) {
        for (idx, cte) in enumerate(&self.ctes) {
            if cte.0.recursive_crawl_all(false).into_iter().any(|it| p0.is(&it)) {
                todo!()
            }
        }
    }
}

impl CTEBuilder {
    fn list_used_names(&self) -> Vec<SmolStr> {
        todo!()
    }

    fn has_duplicate_aliases(&self) -> bool {
        let used_names = self.list_used_names();
        used_names.into_iter().all_unique()
    }

    fn create_cte_alias(&mut self, alias: Option<&AliasInfo>) -> (SmolStr, bool) {
        if let Some(alias) = alias
            && alias.aliased
            && !alias.ref_str.is_empty()
        {
            return (alias.ref_str.clone(), false);
        }

        self.name_idx += 1;
        let name = format_smolstr!("prep_{}", self.name_idx);
        if self.list_used_names().iter().contains(&name) {
            return todo!();
        }

        (name, true)
    }

    fn insert_cte(&mut self, cte: CTEDefinitionSegment) {
        let inbound_subquery = Segments::new(cte.0.clone(), None)
            .children(None)
            .find_first(Some(|it: &ErasedSegment| it.get_position_marker().is_some()));
        let insert_position = self
            .ctes
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                is_child(
                    Segments::new(
                        Segments::new(it.0.clone(), None).children(None).last().cloned().unwrap(),
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

    if child_markers < parent_pos.start_point_marker() {
        return false;
    }

    if child_markers > parent_pos.end_point_marker() {
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
    let first_keyword = root_segment.recursive_crawl(&["keyword"], false, None, true)[0].clone();

    if first_keyword.raw().chars().all(char::is_lowercase) { Case::Lower } else { Case::Upper }
}

fn segmentify(input_el: &str, casing: Case) -> ErasedSegment {
    let mut input_el = input_el.to_lowercase_smolstr();
    if casing == Case::Upper {
        input_el = input_el.to_uppercase_smolstr();
    }
    KeywordSegment::new(input_el, None).to_erased_segment()
}

fn create_cte_seg(
    alias_name: SmolStr,
    subquery: ErasedSegment,
    case_preference: Case,
    dialect: &Dialect,
) -> ErasedSegment {
    Node::new(
        dialect.name,
        SyntaxKind::CommonTableExpression,
        vec![
            IdentifierSegment::create(
                &alias_name,
                None,
                CodeSegmentNewArgs {
                    code_type: "naked_identifier",
                    ..CodeSegmentNewArgs::default()
                },
            ),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
            segmentify("AS", case_preference),
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
            subquery,
        ],
        false,
    )
    .to_erased_segment()
}

fn create_table_ref(table_name: &str, dialect: &Dialect) -> ErasedSegment {
    Node::new(
        dialect.name,
        SyntaxKind::TableExpression,
        vec![
            Node::new(
                dialect.name,
                SyntaxKind::TableReference,
                vec![IdentifierSegment::create(
                    table_name,
                    None,
                    CodeSegmentNewArgs { code_type: "naked_identifier", ..<_>::default() },
                )],
                false,
            )
            .to_erased_segment(),
        ],
        false,
    )
    .to_erased_segment()
}

pub struct SegmentCloneMap {
    segment_map: AHashMap<usize, ErasedSegment>,
}

impl Index<&ErasedSegment> for SegmentCloneMap {
    type Output = ErasedSegment;

    fn index(&self, index: &ErasedSegment) -> &Self::Output {
        &self.segment_map[&index.addr()]
    }
}

impl IndexMut<&ErasedSegment> for SegmentCloneMap {
    fn index_mut(&mut self, index: &ErasedSegment) -> &mut Self::Output {
        self.segment_map.get_mut(&index.addr()).unwrap()
    }
}

impl SegmentCloneMap {
    fn new(segment: ErasedSegment) -> Self {
        let segment_copy = segment.copy();
        let mut segment_map = AHashMap::new();

        for (old_segment, new_segment) in
            zip(segment.recursive_crawl_all(false), segment_copy.recursive_crawl_all(false))
        {
            segment_map.insert(old_segment.addr(), new_segment);
        }

        Self { segment_map }
    }
}
