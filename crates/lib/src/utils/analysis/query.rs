use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use super::select::SelectStatementColumnsAndTables;
use crate::core::dialects::base::Dialect;
use crate::core::parser::segments::base::Segment;
use crate::utils::analysis::select::get_select_statement_info;

static SELECTABLE_TYPES: &[&str] =
    &["with_compound_statement", "set_expression", "select_statement"];

static SUBSELECT_TYPES: &[&str] = &[
    "merge_statement",
    "update_statement",
    "delete_statement",
    // NOTE: Values clauses won't have sub selects, but it's
    // also harmless to look, and they may appear in similar
    // locations. We include them here because they come through
    // the same code paths - although are likely to return nothing.
    "values_clause",
];

pub enum QueryType {
    Simple,
    WithCompound,
}

pub enum WildcardInfo {}

pub struct Selectable<'me> {
    pub selectable: Box<dyn Segment>,
    pub dialect: &'me Dialect,
}

impl<'me> Selectable<'me> {
    pub fn select_info(&self) -> Option<SelectStatementColumnsAndTables> {
        if self.selectable.is_type("select_statement") {
            return get_select_statement_info(&self.selectable, self.dialect.into(), false);
        }

        unimplemented!()
    }
}

pub struct Query<'me, T> {
    pub query_type: QueryType,
    pub dialect: &'me Dialect,
    pub selectables: Vec<Selectable<'me>>,
    pub ctes: HashMap<String, Query<'me, T>>,
    pub parent: Option<Box<Query<'me, T>>>,
    pub subqueries: Vec<Query<'me, T>>,
    pub cte_definition_segment: Option<Box<dyn Segment>>,
    pub cte_name_segment: Option<Box<dyn Segment>>,
    pub payload: T,
}

impl<'me, T> Deref for Query<'me, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

impl<'me, T> DerefMut for Query<'me, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.payload
    }
}

impl<T: Default> Query<'_, T> {
    pub fn children_mut(&mut self) -> impl Iterator<Item = &mut Self> {
        self.ctes.values_mut().chain(self.subqueries.iter_mut())
    }

    fn as_dict() {}

    fn lookup_cte() {}

    fn crawl_sources() {}

    fn extract_subqueries<'a>(selectable: &Selectable, dialect: &'a Dialect) -> Vec<Query<'a, T>> {
        let mut acc = Vec::new();

        for subselect in selectable.selectable.recursive_crawl(SELECTABLE_TYPES, false, None, false)
        {
            acc.push(Query::from_segment(&subselect, dialect, None));
        }

        acc
    }

    fn from_root() {}

    pub fn from_segment<'a>(
        segment: &Box<dyn Segment>,
        dialect: &'a Dialect,
        parent: Option<Query<'a, T>>,
    ) -> Query<'a, T> {
        let mut selectables = Vec::new();
        let mut subqueries = Vec::new();
        let mut cte_defs: Vec<Box<dyn Segment>> = Vec::new();
        let mut query_type = QueryType::Simple;

        if segment.is_type("select_statement")
            || SUBSELECT_TYPES.iter().any(|ty| segment.is_type(ty))
        {
            selectables.push(Selectable { selectable: segment.clone(), dialect });
        } else if segment.is_type("set_expression") {
            unimplemented!()
        } else {
            unimplemented!()
        }

        for selectable in &selectables {
            subqueries.extend(Self::extract_subqueries(selectable, dialect));
        }

        let outer_query = Query {
            query_type,
            dialect,
            selectables,
            ctes: <_>::default(),
            parent: parent.map(Box::new),
            subqueries,
            cte_definition_segment: None,
            cte_name_segment: None,
            payload: T::default(),
        };

        if cte_defs.is_empty() {
            return outer_query;
        }

        outer_query
    }
}
