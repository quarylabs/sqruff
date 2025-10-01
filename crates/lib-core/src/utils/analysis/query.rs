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
pub struct Query<'me> {
    pub query_type: QueryType,
    pub dialect: &'me Dialect,
    pub selectables: Vec<Selectable<'me>>,
    pub ctes: IndexMap<SmolStr, ErasedSegment>,
}

impl<'me> Query<'me> {
    pub fn crawl_sources(
        &self,
        segment: ErasedSegment,

        _pop: bool,
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
                    if let Some(cte) = self.lookup_cte(seg.raw().as_ref()) {
                        acc.push(Source::Query(cte));
                        continue;
                    }
                }
                acc.push(Source::TableReference(seg.raw().clone()));
            } else {
                acc.push(Source::Query(Query::from_segment(&seg, self.dialect)))
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

    #[track_caller]
    pub fn lookup_cte(&self, name: &str) -> Option<Query<'me>> {
        let key = name.to_uppercase_smolstr();
        let seg = self.ctes.get(&key)?.clone();
        // Dive into the CTE definition to find the inner query (SELECT/SET/WITH)
        let inner = seg.recursive_crawl(
            const { &SELECTABLE_TYPES.union(&SUBSELECT_TYPES) },
            true,
            &SyntaxSet::EMPTY,
            true,
        );
        inner.first().map(|qseg| Query::from_segment(qseg, self.dialect))
    }
}

impl Query<'_> {
    pub fn children(&self) -> Vec<Self> {
        let mut acc = Vec::new();
        for selectable in &self.selectables {
            acc.extend(Self::extract_subqueries(selectable, self.dialect));
        }
        acc
    }

    fn extract_subqueries<'a>(selectable: &Selectable, dialect: &'a Dialect) -> Vec<Query<'a>> {
        let mut acc = Vec::new();

        for subselect in selectable.selectable.recursive_crawl(
            &SELECTABLE_TYPES,
            false,
            &SyntaxSet::EMPTY,
            false,
        ) {
            acc.push(Query::from_segment(&subselect, dialect));
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

        Some(Query::from_segment(selectable_segment, dialect))
    }

    pub fn from_segment<'a>(segment: &ErasedSegment, dialect: &'a Dialect) -> Query<'a> {
        let mut selectables = Vec::new();
        let mut cte_defs: IndexMap<SmolStr, ErasedSegment> = IndexMap::default();
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
                let name_seg = seg.segments()[0].clone();
                let name = name_seg.raw().to_uppercase_smolstr();
                cte_defs.insert(name, seg);
            }
        }

        Query {
            query_type,
            dialect,
            selectables,
            ctes: cte_defs,
        }
    }
}

pub enum Source<'a> {
    TableReference(SmolStr),
    Query(Query<'a>),
}
