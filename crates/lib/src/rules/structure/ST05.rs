use ahash::{AHashMap, AHashSet};
use itertools::{enumerate, Itertools};
use smol_str::{format_smolstr, SmolStr, ToSmolStr};

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::{CTEDefinitionSegment, ObjectReferenceLevel};
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
            parent_stack.all(Some(|it: &ErasedSegment| SELECT_TYPES.contains(&it.get_type())));

        if !is_select || is_select_child {
            return Vec::new();
        }

        let query: Query<'_, ()> = Query::from_segment(&context.segment, context.dialect, None);
        let mut ctes = CTEBuilder::default();
        for cte in query.inner.borrow().ctes.values() {
            todo!();
            // ctes.insert_cte(cte.inner.borrow().cte_definition_segment.);
        }

        let is_with = false;
        let is_recursive = is_with
            && !segment.children(Some(|it: &ErasedSegment| it.is_keyword("recursive"))).is_empty();
        if is_with {
            unimplemented!()
        }

        let case_preference = get_case_preference(&segment);
        let output_select = if is_with { todo!() } else { segment };

        self.lint_query(context.dialect, query, ctes, case_preference);

        Vec::new()
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
        mut ctes: CTEBuilder,
        case_preference: Case,
    ) {
        for nsq in self.nested_subqueries(query, dialect) {
            let (alias_name, _) = ctes.create_cte_alias(Some(nsq.table_alias));
        }
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

                    if parent_types
                        .iter()
                        .any(|typ| table_alias.from_expression_element.is_type(typ))
                        || path_to
                            .iter()
                            .any(|ps| parent_types.iter().any(|typ| ps.segment.is_type(typ)))
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
                        query: query.clone(),
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
    fn list_used_names(&self) -> Vec<SmolStr> {
        todo!()
    }

    fn create_cte_alias(&mut self, alias: Option<AliasInfo>) -> (SmolStr, bool) {
        if let Some(alias) = alias
            && alias.aliased
            && !alias.ref_str.is_empty()
        {
            return (alias.ref_str, false);
        }

        self.name_idx += 1;
        let name = format_smolstr!("prep_{}", self.name_idx);
        if self.list_used_names().iter().contains(&name) {
            return todo!();
        }

        (name, true)
    }

    fn has_duplicate_aliases() {}

    fn insert_cte(&mut self, cte: CTEDefinitionSegment) {}
}

fn get_case_preference(root_select: &Segments) -> Case {
    let root_segment = root_select.first().expect("Root SELECT not found");
    let first_keyword = root_segment.recursive_crawl(&["keyword"], false, None, true)[0].clone();

    if first_keyword.raw().chars().all(char::is_lowercase) { Case::Lower } else { Case::Upper }
}

enum Case {
    Lower,
    Upper,
}
