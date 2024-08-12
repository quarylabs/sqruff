use itertools::Itertools;
use smol_str::SmolStr;

use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::core::parser::segments::base::ErasedSegment;
use crate::dialects::ansi::{
    FromClauseSegment, JoinClauseSegment, ObjectReferenceSegment, SelectClauseElementSegment,
};
use crate::dialects::{SyntaxKind, SyntaxSet};

#[derive(Clone)]
pub struct SelectStatementColumnsAndTables {
    pub select_statement: ErasedSegment,
    pub table_aliases: Vec<AliasInfo>,
    pub standalone_aliases: Vec<SmolStr>,
    pub reference_buffer: Vec<ObjectReferenceSegment>,
    pub select_targets: Vec<SelectClauseElementSegment>,
    pub col_aliases: Vec<ColumnAliasInfo>,
    pub using_cols: Vec<SmolStr>,
}

pub fn get_object_references(segment: &ErasedSegment) -> Vec<ObjectReferenceSegment> {
    segment
        .recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::ColumnReference]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        )
        .into_iter()
        .map(|seg| seg.reference())
        .collect()
}

pub fn get_select_statement_info(
    segment: &ErasedSegment,
    dialect: Option<&Dialect>,
    early_exit: bool,
) -> Option<SelectStatementColumnsAndTables> {
    let (table_aliases, standalone_aliases) = get_aliases_from_select(segment, dialect);

    if early_exit && table_aliases.is_empty() && standalone_aliases.is_empty() {
        return None;
    }

    let sc = segment.child(const { &SyntaxSet::new(&[SyntaxKind::SelectClause]) })?;
    let mut reference_buffer = get_object_references(&sc);
    for potential_clause in [
        SyntaxKind::WhereClause,
        SyntaxKind::GroupbyClause,
        SyntaxKind::HavingClause,
        SyntaxKind::OrderbyClause,
        SyntaxKind::QualifyClause,
    ] {
        let clause = segment.child(&SyntaxSet::new(&[potential_clause]));
        if let Some(clause) = clause {
            reference_buffer.extend(get_object_references(&clause));
        }
    }

    let select_clause =
        segment.child(const { &SyntaxSet::new(&[SyntaxKind::SelectClause]) }).unwrap();
    let select_targets =
        select_clause.children(const { &SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) });
    let select_targets =
        select_targets.iter().map(|it| SelectClauseElementSegment(it.clone())).collect_vec();

    let col_aliases = select_targets.iter().flat_map(|s| s.alias()).collect_vec();

    let mut using_cols: Vec<SmolStr> = Vec::new();
    let fc = segment.child(const { &SyntaxSet::new(&[SyntaxKind::FromClause]) });

    if let Some(fc) = fc {
        for join_clause in fc.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            let mut seen_using = false;

            for seg in join_clause.segments() {
                if seg.is_type(SyntaxKind::Keyword) && seg.get_raw_upper().unwrap() == "USING" {
                    seen_using = true;
                } else if seg.is_type(SyntaxKind::JoinOnCondition) {
                    for on_seg in seg.segments() {
                        if matches!(
                            on_seg.get_type(),
                            SyntaxKind::Bracketed | SyntaxKind::Expression
                        ) {
                            reference_buffer.extend(get_object_references(seg));
                        }
                    }
                } else if seen_using && seg.is_type(SyntaxKind::Bracketed) {
                    for subseg in seg.segments() {
                        if subseg.is_type(SyntaxKind::Identifier)
                            || subseg.is_type(SyntaxKind::NakedIdentifier)
                        {
                            using_cols.push(subseg.raw().into());
                        }
                    }
                    seen_using = false;
                }
            }
        }
    }

    SelectStatementColumnsAndTables {
        select_statement: segment.clone(),
        table_aliases,
        standalone_aliases,
        reference_buffer,
        select_targets,
        col_aliases,
        using_cols,
    }
    .into()
}

pub fn get_aliases_from_select(
    segment: &ErasedSegment,
    dialect: Option<&Dialect>,
) -> (Vec<AliasInfo>, Vec<SmolStr>) {
    let fc = segment.child(const { &SyntaxSet::new(&[SyntaxKind::FromClause]) });
    let Some(fc) = fc else {
        return (Vec::new(), Vec::new());
    };

    let aliases = if fc.is_type(SyntaxKind::FromClause) {
        FromClauseSegment(fc).eventual_aliases()
    } else if fc.is_type(SyntaxKind::JoinClause) {
        JoinClauseSegment(fc).eventual_aliases()
    } else {
        unimplemented!()
    };

    let mut standalone_aliases = Vec::new();
    standalone_aliases.extend(get_pivot_table_columns(segment, dialect));
    standalone_aliases.extend(get_lambda_argument_columns(segment, dialect));

    let mut table_aliases = Vec::new();
    for (table_expr, alias_info) in aliases {
        if has_value_table_function(table_expr, dialect) {
            if !standalone_aliases.contains(&alias_info.ref_str) {
                standalone_aliases.push(alias_info.ref_str);
            }
        } else if !table_aliases.contains(&alias_info) {
            table_aliases.push(alias_info);
        }
    }

    (table_aliases, standalone_aliases)
}

fn has_value_table_function(table_expr: ErasedSegment, dialect: Option<&Dialect>) -> bool {
    let Some(dialect) = dialect else {
        return false;
    };

    for function_name in table_expr.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::FunctionName]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        if dialect.sets("value_table_functions").contains(function_name.raw().to_uppercase().trim())
        {
            return true;
        }
    }

    false
}

fn get_pivot_table_columns(segment: &ErasedSegment, dialect: Option<&Dialect>) -> Vec<SmolStr> {
    let Some(_dialect) = dialect else {
        return Vec::new();
    };

    let fc = segment.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::FromPivotExpression]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    );
    if !fc.is_empty() {
        return Vec::new();
    }

    let mut pivot_table_column_aliases = Vec::new();
    for pivot_table_column_alias in segment.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::PivotColumnReference]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        let raw = pivot_table_column_alias.raw().into();
        if !pivot_table_column_aliases.contains(&raw) {
            pivot_table_column_aliases.push(raw);
        }
    }

    pivot_table_column_aliases
}

fn get_lambda_argument_columns(
    _segment: &ErasedSegment,
    _dialect: Option<&Dialect>,
) -> Vec<SmolStr> {
    Vec::new()
}
