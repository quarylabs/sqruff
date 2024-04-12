use std::ops::{Deref, DerefMut};

use ahash::AHashMap;

use super::select::SelectStatementColumnsAndTables;
use crate::core::dialects::base::Dialect;
use crate::core::parser::segments::base::ErasedSegment;
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

#[derive(Debug)]
pub enum QueryType {
    Simple,
    WithCompound,
}

pub enum WildcardInfo {}

#[derive(Debug)]
pub struct Selectable<'me> {
    pub selectable: ErasedSegment,
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

#[derive(Debug)]
pub struct Query<'me, T> {
    pub query_type: QueryType,
    pub dialect: &'me Dialect,
    pub selectables: Vec<Selectable<'me>>,
    pub ctes: AHashMap<String, Query<'me, T>>,
    pub parent: Option<Box<Query<'me, T>>>,
    pub subqueries: Vec<Query<'me, T>>,
    pub cte_definition_segment: Option<ErasedSegment>,
    pub cte_name_segment: Option<ErasedSegment>,
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

    pub fn from_root<'a>(root_segment: ErasedSegment, dialect: &'a Dialect) -> Query<'a, T> {
        let selectable_segment =
            root_segment.recursive_crawl(SELECTABLE_TYPES, true, "merge_statement".into(), true)[0]
                .clone();

        Query::from_segment(&selectable_segment, dialect, None)
    }

    pub fn from_segment<'a>(
        segment: &ErasedSegment,
        dialect: &'a Dialect,
        parent: Option<Query<'a, T>>,
    ) -> Query<'a, T> {
        let mut selectables = Vec::new();
        let mut subqueries = Vec::new();
        let mut cte_defs: Vec<ErasedSegment> = Vec::new();
        let mut query_type = QueryType::Simple;

        if segment.is_type("select_statement")
            || SUBSELECT_TYPES.iter().any(|ty| segment.is_type(ty))
        {
            selectables.push(Selectable { selectable: segment.clone(), dialect });
        } else if segment.is_type("set_expression") {
            unimplemented!()
        } else {
            query_type = QueryType::WithCompound;

            for seg in segment.recursive_crawl(
                &["select_statement"],
                false,
                "common_table_expression".into(),
                true,
            ) {
                selectables.push(Selectable { selectable: seg, dialect });
            }

            for seg in segment.recursive_crawl(
                &["common_table_expression"],
                false,
                "with_compound_statement".into(),
                true,
            ) {
                cte_defs.push(seg);
            }
        }

        for selectable in &selectables {
            subqueries.extend(Self::extract_subqueries(selectable, dialect));
        }

        let mut outer_query = Query {
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

        let mut ctes = AHashMap::new();
        for cte in cte_defs {
            let name_seg = cte.segments()[0].clone_box();
            let name = name_seg.get_raw_upper().unwrap();

            let types = [SELECTABLE_TYPES, &["values_clause"], SUBSELECT_TYPES].concat();
            let queries = cte.recursive_crawl(&types, true, None, true);

            if queries.is_empty() {
                continue;
            };

            let query = &queries[0];
            let mut query = Self::from_segment(query, dialect, None);
            query.cte_definition_segment = cte.into();
            query.cte_name_segment = name_seg.into();
            ctes.insert(name, query);
        }

        outer_query.ctes = ctes;
        outer_query
    }
}
