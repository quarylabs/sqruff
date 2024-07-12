use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;

use crate::core::config::Value;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::ansi::CTEDefinitionSegment;
use crate::utils::analysis::query::{Query, Selectable};
use crate::utils::functional::context::FunctionalContext;

const SELECT_TYPES: [&str; 3] = ["with_compound_statement", "set_expression", "select_statement"];

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

        let query: Query<'_, ()> =
            Query::from_segment(&context.segment, context.dialect.into(), None);
        let mut ctes = CTEBuilder::default();
        for cte in query.inner.borrow().ctes.values() {
            todo!();
            // ctes.insert_cte(cte.inner.borrow().cte_definition_segment.);
        }

        let is_with = false;
        if is_with {
            unimplemented!()
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(SELECT_TYPES.into()).into()
    }
}

impl RuleST05 {
    fn lint_query(
        &self,
        dialect: &Dialect,
        query: Query<'_, ()>,
        ctes: CTEBuilder,
        case_preference: &'static str,
    ) {
        unimplemented!()
    }
}

fn get_first_select_statement_descendant() {}

fn is_correlated_subquery() {}

#[derive(Default)]
struct CTEBuilder {
    ctes: Vec<CTEDefinitionSegment>,
    name_idx: usize,
}

impl CTEBuilder {
    fn list_used_names() {}

    fn has_duplicate_aliases() {}

    fn insert_cte(&mut self, cte: CTEDefinitionSegment) {}
}
