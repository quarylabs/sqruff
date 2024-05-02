use std::cell::RefCell;
use std::rc::Rc;

use ahash::AHashMap;

use super::select::SelectStatementColumnsAndTables;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::AliasInfo;
use crate::core::parser::segments::base::ErasedSegment;
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::segments::Segments;

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

#[derive(Debug, Clone, Copy)]
pub enum QueryType {
    Simple,
    WithCompound,
}

pub enum WildcardInfo {}

#[derive(Debug, Clone)]
pub struct Selectable<'me> {
    pub selectable: ErasedSegment,
    pub dialect: &'me Dialect,
}

impl<'me> Selectable<'me> {
    pub fn select_info(&self) -> Option<SelectStatementColumnsAndTables> {
        if self.selectable.is_type("select_statement") {
            return get_select_statement_info(&self.selectable, self.dialect.into(), false);
        }

        let values = Segments::new(self.selectable.clone(), None);
        let alias_expression = values
            .children(None)
            .find_first(Some(|it: &ErasedSegment| it.is_type("alias_expression")));
        let name = alias_expression.children(None).find_first(Some(|it: &ErasedSegment| {
            matches!(it.get_type(), "naked_identifier" | "quoted_identifier",)
        }));

        let alias_info = AliasInfo {
            ref_str: if name.is_empty() {
                String::new()
            } else {
                name.first().unwrap().get_raw().unwrap()
            },
            segment: name.first().cloned(),
            aliased: !name.is_empty(),
            from_expression_element: self.selectable.clone(),
            alias_expression: alias_expression.first().cloned(),
            object_reference: None,
        };

        SelectStatementColumnsAndTables {
            select_statement: self.selectable.clone(),
            table_aliases: vec![alias_info],
            standalone_aliases: Vec::new(),
            reference_buffer: Vec::new(),
            select_targets: Vec::new(),
            col_aliases: Vec::new(),
            using_cols: Vec::new(),
        }
        .into()
    }
}

#[derive(Debug, Clone)]
pub struct Query<'me, T> {
    pub(crate) inner: Rc<RefCell<QueryInner<'me, T>>>,
}

#[derive(Debug, Clone)]
pub struct QueryInner<'me, T> {
    pub query_type: QueryType,
    pub dialect: &'me Dialect,
    pub selectables: Vec<Selectable<'me>>,
    pub ctes: AHashMap<String, Query<'me, T>>,
    pub parent: Option<Query<'me, T>>,
    pub subqueries: Vec<Query<'me, T>>,
    pub cte_definition_segment: Option<ErasedSegment>,
    pub cte_name_segment: Option<ErasedSegment>,
    pub payload: T,
}

impl<'me, T: Clone> Query<'me, T> {
    fn post_init(&self) {
        let parent = self.clone();

        for subquery in &RefCell::borrow(&self.inner).subqueries {
            RefCell::borrow_mut(&subquery.inner).parent = parent.clone().into();
        }

        for cte in RefCell::borrow(&self.inner).ctes.values().cloned() {
            RefCell::borrow_mut(&cte.inner).parent = parent.clone().into();
        }
    }
}

impl<T: Default + Clone> Query<'_, T> {
    pub fn children(&self) -> Vec<Self> {
        self.inner
            .borrow()
            .ctes
            .values()
            .chain(self.inner.borrow().subqueries.iter())
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    fn as_dict() {}

    #[allow(dead_code)]
    fn lookup_cte() {}

    #[allow(dead_code)]
    fn crawl_sources() {}

    fn extract_subqueries<'a>(selectable: &Selectable, dialect: &'a Dialect) -> Vec<Query<'a, T>> {
        let mut acc = Vec::new();

        for subselect in selectable.selectable.recursive_crawl(SELECTABLE_TYPES, false, None, false)
        {
            acc.push(Query::from_segment(&subselect, dialect, None));
        }

        acc
    }

    pub fn from_root(root_segment: ErasedSegment, dialect: &Dialect) -> Query<'_, T> {
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
            selectables.extend(
                segment
                    .children(&["select_statement"])
                    .into_iter()
                    .map(|selectable| Selectable { selectable, dialect }),
            )
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

        let outer_query = Query {
            inner: Rc::new(RefCell::new(QueryInner {
                query_type,
                dialect,
                selectables,
                ctes: <_>::default(),
                parent,
                subqueries,
                cte_definition_segment: None,
                cte_name_segment: None,
                payload: T::default(),
            })),
        };

        outer_query.post_init();

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
            let query = Self::from_segment(query, dialect, outer_query.clone().into());

            RefCell::borrow_mut(&query.inner).cte_definition_segment = cte.into();
            RefCell::borrow_mut(&query.inner).cte_name_segment = name_seg.into();

            ctes.insert(name, query);
        }

        RefCell::borrow_mut(&outer_query.inner).ctes = ctes;
        outer_query
    }
}
