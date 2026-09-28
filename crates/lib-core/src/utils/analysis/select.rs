use std::collections::HashSet;

use itertools::Itertools;
use smol_str::{SmolStr, ToSmolStr};

use crate::dialects::Dialect;
use crate::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::dialects::init::DialectKind;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::parser::segments::ErasedSegment;
use crate::parser::segments::from::FromClauseSegment;
use crate::parser::segments::join::JoinClauseSegment;
use crate::parser::segments::object_reference::ObjectReferenceSegment;
use crate::parser::segments::select::SelectClauseElementSegment;

#[derive(Clone)]
pub struct SelectStatementColumnsAndTables {
    pub select_statement: ErasedSegment,
    pub table_aliases: Vec<AliasInfo>,
    pub standalone_aliases: Vec<SmolStr>,
    pub reference_buffer: Vec<ObjectReferenceSegment>,
    pub select_targets: Vec<SelectClauseElementSegment>,
    pub col_aliases: Vec<ColumnAliasInfo>,
    pub using_cols: Vec<SmolStr>,
    pub table_reference_buffer: Vec<ObjectReferenceSegment>,
}

fn get_struct_alias_refs(segment: &ErasedSegment) -> HashSet<u32> {
    let mut struct_alias_ids = HashSet::new();

    for function in segment.recursive_crawl(
        const { &SyntaxSet::single(SyntaxKind::Function) },
        true,
        const { &SyntaxSet::new(&[SyntaxKind::SelectStatement, SyntaxKind::MergeStatement]) },
        true,
    ) {
        let Some(function_name) =
            function.child(const { &SyntaxSet::single(SyntaxKind::FunctionName) })
        else {
            continue;
        };
        if !function_name.raw().eq_ignore_ascii_case("STRUCT") {
            continue;
        }

        let Some(function_contents) =
            function.child(const { &SyntaxSet::single(SyntaxKind::FunctionContents) })
        else {
            continue;
        };
        let Some(bracketed) =
            function_contents.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
        else {
            continue;
        };

        let mut previous_was_alias_marker = false;
        for segment in bracketed.segments() {
            if segment.is_type(SyntaxKind::IndexColumnDefinition) {
                previous_was_alias_marker = true;
            } else if segment.is_type(SyntaxKind::Expression)
                && segment.raw().eq_ignore_ascii_case("AS")
            {
                // sqruff currently parses the STRUCT alias marker itself as an
                // expression/object reference. Exclude it along with the alias
                // expression that follows it.
                for reference in segment.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::ColumnReference])
                    },
                    true,
                    const { &SyntaxSet::EMPTY },
                    true,
                ) {
                    struct_alias_ids.insert(reference.id());
                }
                previous_was_alias_marker = true;
            } else if previous_was_alias_marker
                && !matches!(
                    segment.get_type(),
                    SyntaxKind::Whitespace
                        | SyntaxKind::Newline
                        | SyntaxKind::Indent
                        | SyntaxKind::Dedent
                )
            {
                for reference in segment.recursive_crawl(
                    const {
                        &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::ColumnReference])
                    },
                    true,
                    const { &SyntaxSet::EMPTY },
                    true,
                ) {
                    struct_alias_ids.insert(reference.id());
                }
                previous_was_alias_marker = false;
            }
        }
    }

    struct_alias_ids
}

fn get_object_references_excluding(
    segment: &ErasedSegment,
    exclude_ids: Option<&HashSet<u32>>,
) -> Vec<ObjectReferenceSegment> {
    segment
        .recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::ColumnReference]) },
            true,
            const { &SyntaxSet::new(&[SyntaxKind::SelectStatement, SyntaxKind::MergeStatement]) },
            true,
        )
        .into_iter()
        .filter(|segment| exclude_ids.is_none_or(|ids| !ids.contains(&segment.id())))
        .map(|seg| seg.reference())
        .collect()
}

pub fn get_object_references(segment: &ErasedSegment) -> Vec<ObjectReferenceSegment> {
    get_object_references_excluding(segment, None)
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
    let struct_alias_ids = get_struct_alias_refs(&sc);
    let mut reference_buffer = get_object_references_excluding(&sc, Some(&struct_alias_ids));
    let mut table_reference_buffer = Vec::new();
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

    let select_clause = segment
        .child(const { &SyntaxSet::new(&[SyntaxKind::SelectClause]) })
        .unwrap();
    let select_targets =
        select_clause.children(const { &SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) });
    let select_targets = select_targets
        .map(|it| SelectClauseElementSegment(it.clone()))
        .collect_vec();

    let col_aliases = select_targets
        .iter()
        .filter_map(|s| s.alias())
        .collect_vec();

    let mut using_cols: Vec<SmolStr> = Vec::new();
    let fc = segment.child(const { &SyntaxSet::new(&[SyntaxKind::FromClause]) });

    if let Some(fc) = fc {
        for table_expression in fc.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            for seg in table_expression.segments() {
                if !seg.is_type(SyntaxKind::TableReference) {
                    reference_buffer.extend(get_object_references(seg));
                } else if seg.reference().is_qualified() {
                    table_reference_buffer.extend(get_object_references(seg));
                }
            }
        }

        for join_clause in fc.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            true,
        ) {
            let mut seen_using = false;

            for seg in join_clause.segments() {
                if seg.is_keyword("USING") {
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
                            using_cols.push(subseg.raw().clone());
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
        table_reference_buffer,
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
    standalone_aliases.extend(get_pivot_table_aliases(segment, dialect));
    standalone_aliases.extend(get_lambda_argument_columns(segment, dialect));
    standalone_aliases.extend(get_unpivot_table_aliases(segment, dialect));

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
        const { &SyntaxSet::new(&[SyntaxKind::SelectStatement]) },
        true,
    ) {
        if dialect
            .sets("value_table_functions")
            .contains(function_name.raw().to_uppercase().trim())
        {
            return true;
        }
    }

    false
}

fn get_pivot_table_aliases(segment: &ErasedSegment, dialect: Option<&Dialect>) -> Vec<SmolStr> {
    let Some(_dialect) = dialect else {
        return Vec::new();
    };

    let mut aliases = Vec::new();
    for pivot in segment.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::FromPivotExpression]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        for alias in
            pivot.recursive_crawl(
                const {
                    &SyntaxSet::new(&[SyntaxKind::PivotColumnReference, SyntaxKind::TableReference])
                },
                true,
                &SyntaxSet::EMPTY,
                true,
            )
        {
            // Standalone aliases are strings here, so preserve both spellings
            // before discarding the segment used to normalize quoted identifiers.
            for name in [alias.raw().clone(), alias.raw_normalized()] {
                if !aliases.contains(&name) {
                    aliases.push(name);
                }
            }
        }
    }

    aliases
}

fn push_standalone_alias(aliases: &mut Vec<SmolStr>, alias: &ErasedSegment) {
    // Standalone aliases are strings here, so preserve both spellings before
    // discarding the segment used to normalize quoted identifiers.
    for name in [alias.raw().clone(), alias.raw_normalized()] {
        if !aliases.contains(&name) {
            aliases.push(name);
        }
    }
}

fn get_unpivot_table_aliases(segment: &ErasedSegment, dialect: Option<&Dialect>) -> Vec<SmolStr> {
    let Some(_dialect) = dialect else {
        return Vec::new();
    };

    let mut aliases = Vec::new();

    // Redshift SUPER object unpivoting and array unnesting introduce output
    // aliases immediately after their AS and AT keywords.
    for unpivot in segment.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::ObjectUnpivoting, SyntaxKind::ArrayUnnesting]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        let mut seen_keyword = false;
        for child in unpivot.segments() {
            if child.is_keyword("AS") || child.is_keyword("AT") {
                seen_keyword = true;
            } else if seen_keyword
                && !matches!(
                    child.get_type(),
                    SyntaxKind::Whitespace
                        | SyntaxKind::Newline
                        | SyntaxKind::Indent
                        | SyntaxKind::Dedent
                )
            {
                push_standalone_alias(&mut aliases, child);
                seen_keyword = false;
            }
        }
    }

    // Standard UNPIVOT expressions introduce value and key columns before the
    // IN clause: UNPIVOT (value_column FOR key_column IN (...)).
    for unpivot in segment.recursive_crawl(
        const { &SyntaxSet::single(SyntaxKind::FromUnpivotExpression) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        let Some(bracketed) = unpivot.child(const { &SyntaxSet::single(SyntaxKind::Bracketed) })
        else {
            continue;
        };

        for child in bracketed.segments() {
            if child.is_keyword("IN") {
                break;
            }

            if !matches!(
                child.get_type(),
                SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::Indent
                    | SyntaxKind::Dedent
                    | SyntaxKind::StartBracket
                    | SyntaxKind::EndBracket
                    | SyntaxKind::Bracketed
                    | SyntaxKind::Keyword
                    | SyntaxKind::Comma
            ) {
                push_standalone_alias(&mut aliases, child);
            }
        }
    }

    aliases
}

fn get_lambda_argument_columns(segment: &ErasedSegment, dialect: Option<&Dialect>) -> Vec<SmolStr> {
    let Some(dialect) = dialect else {
        return Vec::new();
    };

    if !matches!(
        dialect.name,
        DialectKind::Athena
            | DialectKind::Sparksql
            | DialectKind::Duckdb
            | DialectKind::Trino
            | DialectKind::Databricks
            | DialectKind::Snowflake
    ) {
        return Vec::new();
    }

    let mut lambda_argument_columns = Vec::new();
    for potential_lambda in segment.recursive_crawl(
        const { &SyntaxSet::new(&[SyntaxKind::Expression, SyntaxKind::LambdaFunction]) },
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        let Some(potential_arrow) = potential_lambda.child(
            const { &SyntaxSet::new(&[SyntaxKind::BinaryOperator, SyntaxKind::LambdaArrow]) },
        ) else {
            continue;
        };

        if potential_arrow.raw() == "->" {
            let arrow_operator = &potential_arrow;
            let mut argument_segments = potential_lambda
                .segments()
                .iter()
                .take_while(|&it| it != arrow_operator)
                .filter(|it| {
                    matches!(
                        it.get_type(),
                        SyntaxKind::Bracketed | SyntaxKind::ColumnReference | SyntaxKind::Parameter
                    )
                })
                .collect_vec();

            assert_eq!(argument_segments.len(), 1);
            let child_segment = argument_segments.pop().unwrap();

            match child_segment.get_type() {
                SyntaxKind::Bracketed => {
                    let start_bracket = child_segment
                        .child(&SyntaxSet::single(SyntaxKind::StartBracket))
                        .unwrap();

                    if start_bracket.raw() == "(" {
                        let bracketed_arguments = child_segment.children(
                            const {
                                &SyntaxSet::new(&[
                                    SyntaxKind::ColumnReference,
                                    SyntaxKind::Parameter,
                                ])
                            },
                        );

                        lambda_argument_columns.extend(
                            bracketed_arguments
                                .into_iter()
                                .map(|argument| argument.raw().to_smolstr()),
                        )
                    }
                }
                SyntaxKind::ColumnReference | SyntaxKind::Parameter => {
                    lambda_argument_columns.push(child_segment.raw().to_smolstr())
                }
                _ => {}
            }
        }
    }

    lambda_argument_columns
}
