use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ahash::AHashSet;
use smol_str::{SmolStr, StrExt, ToSmolStr};

use super::select::SelectStatementColumnsAndTables;
use crate::dialects::Dialect;
use crate::dialects::common::AliasInfo;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::helpers::IndexMap;
use crate::parser::segments::ErasedSegment;
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

static QUERY_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn next_query_id() -> usize {
    QUERY_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        let alias_expression = values
            .children_all()
            .find_first_where(|it: &ErasedSegment| it.is_type(SyntaxKind::AliasExpression));
        let name = alias_expression
            .children_all()
            .find_first_where(|it: &ErasedSegment| {
                matches!(
                    it.get_type(),
                    SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier,
                )
            });

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

/// Internal data structure holding all query fields.
/// This replaces the old QueryInner struct but is now immutable after construction.
#[derive(Debug)]
struct QueryData<'me> {
    id: usize,
    query_type: QueryType,
    dialect: &'me Dialect,
    selectables: Vec<Selectable<'me>>,
    ctes: IndexMap<SmolStr, Query<'me>>,
    parent: Option<Query<'me>>,
    subqueries: Vec<Query<'me>>,
    cte_definition_segment: Option<ErasedSegment>,
    cte_name_segment: Option<ErasedSegment>,
}

/// A flattened Query structure without interior mutability.
/// Uses Rc for shared ownership but no RefCell - the structure is immutable after construction.
#[derive(Debug, Clone)]
pub struct Query<'me> {
    inner: Rc<QueryData<'me>>,
}

impl<'me> Query<'me> {
    /// Returns a unique identifier for this query instance.
    pub fn id(&self) -> usize {
        self.inner.id
    }

    /// Returns the query type.
    pub fn query_type(&self) -> QueryType {
        self.inner.query_type
    }

    /// Returns the dialect.
    pub fn dialect(&self) -> &'me Dialect {
        self.inner.dialect
    }

    /// Returns the selectables in this query.
    pub fn selectables(&self) -> &[Selectable<'me>] {
        &self.inner.selectables
    }

    /// Returns the CTEs defined in this query.
    pub fn ctes(&self) -> &IndexMap<SmolStr, Query<'me>> {
        &self.inner.ctes
    }

    /// Returns the parent query, if any.
    pub fn parent(&self) -> Option<&Query<'me>> {
        self.inner.parent.as_ref()
    }

    /// Returns the subqueries within this query.
    pub fn subqueries(&self) -> &[Query<'me>] {
        &self.inner.subqueries
    }

    /// Returns the CTE definition segment, if this query is a CTE.
    pub fn cte_definition_segment(&self) -> Option<&ErasedSegment> {
        self.inner.cte_definition_segment.as_ref()
    }

    /// Returns the CTE name segment, if this query is a CTE.
    pub fn cte_name_segment(&self) -> Option<&ErasedSegment> {
        self.inner.cte_name_segment.as_ref()
    }

    /// Crawl sources from a segment, optionally looking up CTEs.
    /// When `pop` is true, uses `consumed_ctes` to track which CTEs have been visited.
    pub fn crawl_sources(
        &self,
        segment: ErasedSegment,
        mut consumed_ctes: Option<&mut AHashSet<SmolStr>>,
        lookup_cte: bool,
    ) -> Vec<Source<'me>> {
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
                    let cte = if let Some(consumed) = consumed_ctes.as_deref_mut() {
                        self.lookup_cte_tracked(seg.raw().as_ref(), consumed)
                    } else {
                        self.lookup_cte(seg.raw().as_ref())
                    };
                    if let Some(cte) = cte {
                        acc.push(Source::Query(cte));
                    }
                }
                acc.push(Source::TableReference(seg.raw().clone()));
            } else {
                acc.push(Source::Query(Query::from_segment(
                    &seg,
                    self.inner.dialect,
                    Some(self.clone()),
                )))
            }
        }

        if acc.is_empty()
            && let Some(table_expr) =
                segment.child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) })
        {
            return vec![Source::TableReference(table_expr.raw().to_smolstr())];
        }

        acc
    }

    /// Look up a CTE by name, searching up the parent chain.
    /// Returns a clone of the CTE query if found.
    pub fn lookup_cte(&self, name: &str) -> Option<Query<'me>> {
        let upper_name = name.to_uppercase_smolstr();
        if let Some(cte) = self.inner.ctes.get(&upper_name) {
            return Some(cte.clone());
        }
        self.inner.parent.as_ref().and_then(|p| p.lookup_cte(name))
    }

    /// Look up a CTE by name, tracking which CTEs have been consumed.
    /// This replaces the old `pop` behavior - once a CTE is found and returned,
    /// its name is added to the consumed set so it won't be returned again.
    #[track_caller]
    pub fn lookup_cte_tracked(
        &self,
        name: &str,
        consumed: &mut AHashSet<SmolStr>,
    ) -> Option<Query<'me>> {
        let upper_name = name.to_uppercase_smolstr();

        // Check if already consumed
        if consumed.contains(&upper_name) {
            // Look in parent instead
            return self
                .inner
                .parent
                .as_ref()
                .and_then(|p| p.lookup_cte_tracked(name, consumed));
        }

        // Try to find in our CTEs
        if let Some(cte) = self.inner.ctes.get(&upper_name) {
            consumed.insert(upper_name);
            return Some(cte.clone());
        }

        // Look in parent
        self.inner
            .parent
            .as_ref()
            .and_then(|p| p.lookup_cte_tracked(name, consumed))
    }

    /// Returns all child queries (CTEs and subqueries).
    pub fn children(&self) -> Vec<Self> {
        self.inner
            .ctes
            .values()
            .chain(self.inner.subqueries.iter())
            .cloned()
            .collect()
    }

    fn extract_subqueries<'a>(selectable: &Selectable, dialect: &'a Dialect) -> Vec<Query<'a>> {
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

    pub fn from_root<'a>(root_segment: &ErasedSegment, dialect: &'a Dialect) -> Option<Query<'a>> {
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
        parent: Option<Query<'a>>,
    ) -> Query<'a> {
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

        // Build the outer query first without CTEs
        let outer_query = Query {
            inner: Rc::new(QueryData {
                id: next_query_id(),
                query_type,
                dialect,
                selectables,
                ctes: IndexMap::default(),
                parent: parent.clone(),
                subqueries: subqueries.clone(),
                cte_definition_segment: None,
                cte_name_segment: None,
            }),
        };

        if cte_defs.is_empty() {
            // Set parent references on subqueries
            return set_parent_on_children(outer_query);
        }

        // Build CTEs with parent set to outer_query
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

            let query_seg = &queries[0];
            let cte_query = Self::from_segment(query_seg, dialect, Some(outer_query.clone()));

            // Create a new query with the CTE metadata set
            let cte_query = Query {
                inner: Rc::new(QueryData {
                    id: cte_query.inner.id,
                    query_type: cte_query.inner.query_type,
                    dialect: cte_query.inner.dialect,
                    selectables: cte_query.inner.selectables.clone(),
                    ctes: cte_query.inner.ctes.clone(),
                    parent: Some(outer_query.clone()),
                    subqueries: cte_query.inner.subqueries.clone(),
                    cte_definition_segment: Some(cte),
                    cte_name_segment: Some(name_seg),
                }),
            };

            ctes.insert(name, cte_query);
        }

        // Rebuild outer_query with CTEs included
        let outer_query = Query {
            inner: Rc::new(QueryData {
                id: outer_query.inner.id,
                query_type: outer_query.inner.query_type,
                dialect: outer_query.inner.dialect,
                selectables: outer_query.inner.selectables.clone(),
                ctes,
                parent,
                subqueries,
                cte_definition_segment: None,
                cte_name_segment: None,
            }),
        };

        set_parent_on_children(outer_query)
    }
}

/// Recursively links all queries in the tree to point to the correct final parent.
/// This is called after the entire tree is built to ensure parent chains are correct.
fn set_parent_on_children<'a>(root: Query<'a>) -> Query<'a> {
    // Since we have an immutable structure, we need to rebuild the entire tree
    // with correct parent references. We do this by keeping track of all CTEs
    // at each level and rebuilding from top to bottom.

    // Collect ancestor CTEs from the parent chain (if any)
    let mut ancestor_ctes = IndexMap::default();
    let mut current = root.inner.parent.clone();
    while let Some(p) = current {
        // Add parent's CTEs (don't overwrite - inner CTEs take precedence)
        for (name, cte) in p.ctes().iter() {
            ancestor_ctes.entry(name.clone()).or_insert_with(|| cte.clone());
        }
        current = p.parent().cloned();
    }

    // Add root's own CTEs (these take precedence over ancestor CTEs)
    for (name, cte) in root.inner.ctes.iter() {
        ancestor_ctes.insert(name.clone(), cte.clone());
    }

    // Preserve the parent if it exists
    rebuild_tree_with_cte_context(root.clone(), root.inner.parent.clone(), &ancestor_ctes)
}

/// Recursively rebuilds the query tree, ensuring all queries can access CTEs from their ancestors.
/// The `ancestor_ctes` map accumulates all CTEs that should be visible from this level.
fn rebuild_tree_with_cte_context<'a>(
    query: Query<'a>,
    new_parent: Option<Query<'a>>,
    ancestor_ctes: &IndexMap<SmolStr, Query<'a>>,
) -> Query<'a> {
    // Build merged CTEs for this level (this query's CTEs + ancestor CTEs)
    let mut merged_ctes = ancestor_ctes.clone();
    for (name, cte) in query.inner.ctes.iter() {
        merged_ctes.insert(name.clone(), cte.clone());
    }

    // First create a temporary parent query to pass to children
    let temp_query = Query {
        inner: Rc::new(QueryData {
            id: query.inner.id,
            query_type: query.inner.query_type,
            dialect: query.inner.dialect,
            selectables: query.inner.selectables.clone(),
            ctes: merged_ctes.clone(), // Include all ancestor CTEs for lookups
            parent: new_parent.clone(),
            subqueries: Vec::new(),
            cte_definition_segment: query.inner.cte_definition_segment.clone(),
            cte_name_segment: query.inner.cte_name_segment.clone(),
        }),
    };

    // Recursively rebuild CTEs with this query as parent
    let new_ctes: IndexMap<SmolStr, Query<'a>> = query
        .inner
        .ctes
        .iter()
        .map(|(name, cte)| {
            let new_cte =
                rebuild_tree_with_cte_context(cte.clone(), Some(temp_query.clone()), &merged_ctes);
            (name.clone(), new_cte)
        })
        .collect();

    // Recursively rebuild subqueries with this query as parent
    let new_subqueries: Vec<Query<'a>> = query
        .inner
        .subqueries
        .iter()
        .map(|sq| rebuild_tree_with_cte_context(sq.clone(), Some(temp_query.clone()), &merged_ctes))
        .collect();

    // Create the final query with all children properly set
    // Note: we only include the query's OWN CTEs in its ctes map.
    // Ancestor CTEs are accessible through the parent chain via lookup_cte().
    Query {
        inner: Rc::new(QueryData {
            id: query.inner.id,
            query_type: query.inner.query_type,
            dialect: query.inner.dialect,
            selectables: query.inner.selectables.clone(),
            ctes: new_ctes,
            parent: new_parent,
            subqueries: new_subqueries,
            cte_definition_segment: query.inner.cte_definition_segment.clone(),
            cte_name_segment: query.inner.cte_name_segment.clone(),
        }),
    }
}

pub enum Source<'a> {
    TableReference(SmolStr),
    Query(Query<'a>),
}
