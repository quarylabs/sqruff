use std::cell::RefCell;
use std::rc::Rc;

use smol_str::{SmolStr, StrExt, ToSmolStr};

use super::select::SelectStatementColumnsAndTables;
use crate::dialects::base::Dialect;
use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::helpers::IndexMap;
use crate::parser::segments::base::ErasedSegment;
use crate::utils::analysis::select::get_select_statement_info;
use crate::utils::functional::segments::Segments;

const SELECTABLE_TYPES: SyntaxSet = SyntaxSet::new(&[
    SyntaxKind::WithCompoundStatement,
    SyntaxKind::SetExpression,
    SyntaxKind::SelectStatement,
]);

const SUBSELECT_TYPES: SyntaxSet = SyntaxSet::new(&[
    SyntaxKind::MergeStatement,
    SyntaxKind::UpdateStatement,
    SyntaxKind::DeleteStatement,
    // NOTE: Values clauses won't have sub selects, but it's
    // also harmless to look, and they may appear in similar
    // locations. We include them here because they come through
    // the same code paths - although are likely to return nothing.
    SyntaxKind::ValuesClause,
]);

#[derive(Debug, Clone, Copy)]
pub enum QueryType {
    Simple,
    WithCompound,
}

pub struct WildcardInfo {
    pub segment: ErasedSegment,
    pub tables: Vec<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct Selectable<'me> {
    pub selectable: ErasedSegment,
    pub dialect: &'me Dialect,
}

impl Selectable<'_> {
    pub fn find_alias(&self, table: &str) -> Option<AliasInfo> {
        self.select_info()
            .as_ref()?
            .table_aliases
            .iter()
            .find(|&t| t.aliased && t.ref_str == table)
            .cloned()
    }
}

impl Selectable<'_> {
    pub fn wildcard_info(&self) -> Vec<WildcardInfo> {
        let Some(select_info) = self.select_info() else {
            return Vec::new();
        };

        let mut buff = Vec::new();
        for seg in select_info.select_targets {
            if seg
                .0
                .child(const { &SyntaxSet::new(&[SyntaxKind::WildcardExpression]) })
                .is_some()
            {
                if seg.0.raw().contains('.') {
                    let table = seg
                        .0
                        .raw()
                        .rsplit_once('.')
                        .map(|x| x.0)
                        .unwrap_or_default()
                        .to_smolstr();
                    buff.push(WildcardInfo {
                        segment: seg.0.clone(),
                        tables: vec![table],
                    });
                } else {
                    let tables = select_info
                        .table_aliases
                        .iter()
                        .filter(|it| !it.ref_str.is_empty())
                        .map(|it| {
                            if it.aliased {
                                it.ref_str.clone()
                            } else {
                                it.from_expression_element.raw().clone()
                            }
                        })
                        .collect();
                    buff.push(WildcardInfo {
                        segment: seg.0.clone(),
                        tables,
                    });
                }
            }
        }

        buff
    }
}

impl Selectable<'_> {
    pub fn select_info(&self) -> Option<SelectStatementColumnsAndTables> {
        if self.selectable.is_type(SyntaxKind::SelectStatement) {
            return get_select_statement_info(&self.selectable, self.dialect.into(), false);
        }

        let values = Segments::new(self.selectable.clone(), None);
        let alias_expression = values.children(None).find_first(Some(|it: &ErasedSegment| {
            it.is_type(SyntaxKind::AliasExpression)
        }));
        let name = alias_expression
            .children(None)
            .find_first(Some(|it: &ErasedSegment| {
                matches!(
                    it.get_type(),
                    SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier,
                )
            }));

        let alias_info = AliasInfo {
            ref_str: if name.is_empty() {
                SmolStr::new_static("")
            } else {
                name.first().unwrap().raw().clone()
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
    pub inner: Rc<RefCell<QueryInner<'me, T>>>,
}

#[derive(Debug, Clone)]
pub struct QueryInner<'me, T> {
    pub query_type: QueryType,
    pub dialect: &'me Dialect,
    pub selectables: Vec<Selectable<'me>>,
    pub ctes: IndexMap<SmolStr, Query<'me, T>>,
    pub parent: Option<Query<'me, T>>,
    pub subqueries: Vec<Query<'me, T>>,
    pub cte_definition_segment: Option<ErasedSegment>,
    pub cte_name_segment: Option<ErasedSegment>,
    pub payload: T,
}

impl<'me, T: Clone + Default> Query<'me, T> {
    pub fn crawl_sources(
        &self,
        segment: ErasedSegment,

        pop: bool,
        lookup_cte: bool,
    ) -> Vec<Source<'me, T>> {
        let mut acc = Vec::new();

        for seg in segment.recursive_crawl(
            const {
                &SyntaxSet::new(&[
                    SyntaxKind::TableReference,
                    SyntaxKind::SetExpression,
                    SyntaxKind::SelectStatement,
                    SyntaxKind::ValuesClause,
                ])
            },
            false,
            &SyntaxSet::EMPTY,
            false,
        ) {
            if seg.is_type(SyntaxKind::TableReference) {
                let _seg = seg.reference();
                if !_seg.is_qualified() && lookup_cte {
                    if let Some(cte) = self.lookup_cte(seg.raw().as_ref(), pop) {
                        acc.push(Source::Query(cte));
                    }
                }
                acc.push(Source::TableReference(seg.raw().clone()));
            } else {
                acc.push(Source::Query(Query::from_segment(
                    &seg,
                    self.inner.borrow().dialect,
                    Some(self.clone()),
                )))
            }
        }

        if acc.is_empty() {
            if let Some(table_expr) =
                segment.child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) })
            {
                return vec![Source::TableReference(table_expr.raw().to_smolstr())];
            }
        }

        acc
    }

    #[track_caller]
    pub fn lookup_cte(&self, name: &str, pop: bool) -> Option<Query<'me, T>> {
        let cte = if pop {
            self.inner
                .borrow_mut()
                .ctes
                .shift_remove(&name.to_uppercase_smolstr())
        } else {
            self.inner
                .borrow()
                .ctes
                .get(&name.to_uppercase_smolstr())
                .cloned()
        };

        cte.or_else(move || {
            self.inner
                .borrow_mut()
                .parent
                .as_mut()
                .and_then(|it| it.lookup_cte(name, pop))
        })
    }

    fn post_init(&self) {
        let this = self.clone();

        for subquery in &RefCell::borrow(&self.inner).subqueries {
            RefCell::borrow_mut(&subquery.inner).parent = this.clone().into();
        }

        for cte in RefCell::borrow(&self.inner).ctes.values().cloned() {
            RefCell::borrow_mut(&cte.inner).parent = this.clone().into();
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

    fn extract_subqueries<'a>(selectable: &Selectable, dialect: &'a Dialect) -> Vec<Query<'a, T>> {
        let mut acc = Vec::new();

        for subselect in selectable.selectable.recursive_crawl(
            &SELECTABLE_TYPES,
            false,
            &SyntaxSet::EMPTY,
            false,
        ) {
            acc.push(Query::from_segment(&subselect, dialect, None));
        }

        acc
    }

    pub fn from_root<'a>(
        root_segment: &ErasedSegment,
        dialect: &'a Dialect,
    ) -> Option<Query<'a, T>> {
        let stmts = root_segment.recursive_crawl(
            &SELECTABLE_TYPES,
            true,
            &SyntaxSet::single(SyntaxKind::MergeStatement),
            true,
        );
        let selectable_segment = stmts.first()?;

        Some(Query::from_segment(selectable_segment, dialect, None))
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

        if segment.is_type(SyntaxKind::SelectStatement)
            || SUBSELECT_TYPES.contains(segment.get_type())
        {
            selectables.push(Selectable {
                selectable: segment.clone(),
                dialect,
            });
        } else if segment.is_type(SyntaxKind::SetExpression) {
            selectables.extend(
                segment
                    .children(const { &SyntaxSet::new(&[SyntaxKind::SelectStatement]) })
                    .cloned()
                    .map(|selectable| Selectable {
                        selectable,
                        dialect,
                    }),
            )
        } else {
            query_type = QueryType::WithCompound;

            for seg in segment.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::SelectStatement]) },
                false,
                const { &SyntaxSet::single(SyntaxKind::CommonTableExpression) },
                true,
            ) {
                selectables.push(Selectable {
                    selectable: seg,
                    dialect,
                });
            }

            for seg in segment.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::CommonTableExpression]) },
                false,
                const { &SyntaxSet::single(SyntaxKind::WithCompoundStatement) },
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

        let mut ctes = IndexMap::default();
        for cte in cte_defs {
            let name_seg = cte.segments()[0].clone();
            let name = name_seg.raw().to_uppercase_smolstr();

            let queries = cte.recursive_crawl(
                const { &SELECTABLE_TYPES.union(&SUBSELECT_TYPES) },
                true,
                &SyntaxSet::EMPTY,
                true,
            );

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

pub enum Source<'a, T> {
    TableReference(SmolStr),
    Query(Query<'a, T>),
}
