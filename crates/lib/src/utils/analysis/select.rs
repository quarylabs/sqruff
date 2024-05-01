use itertools::Itertools;

use crate::core::dialects::base::Dialect;
use crate::core::dialects::common::{AliasInfo, ColumnAliasInfo};
use crate::core::parser::segments::base::ErasedSegment;
use crate::dialects::ansi::{
    FromClauseSegment, Node, ObjectReferenceSegment, SelectClauseElementSegment,
};

pub struct SelectStatementColumnsAndTables {
    pub select_statement: ErasedSegment,
    pub table_aliases: Vec<AliasInfo>,
    pub standalone_aliases: Vec<String>,
    pub reference_buffer: Vec<Node<ObjectReferenceSegment>>,
    pub select_targets: Vec<Node<SelectClauseElementSegment>>,
    pub col_aliases: Vec<ColumnAliasInfo>,
    pub using_cols: Vec<String>,
}

pub fn get_object_references(segment: &ErasedSegment) -> Vec<Node<ObjectReferenceSegment>> {
    segment
        .recursive_crawl(
            &["object_reference", "column_reference"],
            true,
            "select_statement".into(),
            true,
        )
        .into_iter()
        .map(|seg| seg.as_object_reference())
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

    let sc = segment.child(&["select_clause"])?;
    let mut reference_buffer = get_object_references(&sc);
    for potential_clause in
        ["where_clause", "groupby_clause", "having_clause", "orderby_clause", "qualify_clause"]
    {
        let clause = segment.child(&[potential_clause]);
        if let Some(clause) = clause {
            reference_buffer.extend(get_object_references(&clause));
        }
    }

    let select_clause = segment.child(&["select_clause"]).unwrap();
    let select_targets = select_clause.children(&["select_clause_element"]);
    let select_targets = select_targets
        .iter()
        .map(|it| it.as_any().downcast_ref::<Node<SelectClauseElementSegment>>().unwrap().clone())
        .collect_vec();

    let col_aliases = select_targets.iter().flat_map(|s| s.alias()).collect_vec();

    let mut using_cols = Vec::new();
    let fc = segment.child(&["from_clause"]);

    if let Some(fc) = fc {
        for join_clause in
            fc.recursive_crawl(&["join_clause"], true, "select_statement".into(), true)
        {
            let mut seen_using = false;

            for seg in join_clause.segments() {
                if seg.is_type("keyword") && seg.get_raw_upper().unwrap() == "USING" {
                    seen_using = true;
                } else if seg.is_type("join_on_condition") {
                    for on_seg in seg.segments() {
                        if matches!(on_seg.get_type(), "bracketed" | "expression") {
                            reference_buffer.extend(get_object_references(seg));
                        }
                    }
                } else if seen_using && seg.is_type("bracketed") {
                    for subseg in seg.segments() {
                        if subseg.is_type("identifier") || subseg.is_type("naked_identifier") {
                            using_cols.push(subseg.get_raw().unwrap());
                        }
                    }
                    seen_using = false;
                }
            }
        }
    }

    SelectStatementColumnsAndTables {
        select_statement: segment.clone_box(),
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
) -> (Vec<AliasInfo>, Vec<String>) {
    let fc = segment.child(&["from_clause"]);
    let Some(fc) = fc else {
        return (Vec::new(), Vec::new());
    };

    let fc = fc.as_any().downcast_ref::<Node<FromClauseSegment>>().unwrap();
    let aliases = fc.eventual_aliases();

    let mut standalone_aliases = Vec::new();
    standalone_aliases.extend(get_pivot_table_columns(segment, dialect));
    standalone_aliases.extend(get_lambda_argument_columns(segment, dialect));

    let mut table_aliases = Vec::new();
    for (table_expr, alias_info) in aliases {
        if has_value_table_function(table_expr, dialect) {
            standalone_aliases.push(alias_info.ref_str);
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

    for function_name in table_expr.recursive_crawl(&["function_name"], true, None, true) {
        if dialect.sets("value_table_functions").contains(function_name.get_raw().unwrap().trim()) {
            return true;
        }
    }

    false
}

fn get_pivot_table_columns(segment: &ErasedSegment, dialect: Option<&Dialect>) -> Vec<String> {
    let Some(_dialect) = dialect else {
        return Vec::new();
    };

    let fc = segment.recursive_crawl(&["from_pivot_expression"], true, None, true);
    if !fc.is_empty() {
        return Vec::new();
    }

    let mut pivot_table_column_aliases = Vec::new();
    for pivot_table_column_alias in
        segment.recursive_crawl(&["pivot_column_reference"], true, None, true)
    {
        let raw = pivot_table_column_alias.get_raw().unwrap();
        if !pivot_table_column_aliases.contains(&raw) {
            pivot_table_column_aliases.push(raw);
        }
    }

    pivot_table_column_aliases
}

fn get_lambda_argument_columns(
    _segment: &ErasedSegment,
    _dialect: Option<&Dialect>,
) -> Vec<String> {
    Vec::new()
}
