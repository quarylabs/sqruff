use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::segments::ErasedSegment;

pub fn identifiers_policy_applicable(policy: &str, parent_stack: &[ErasedSegment]) -> bool {
    match policy {
        "all" => true,
        "none" => false,
        _ => {
            let is_alias = parent_stack.iter().any(|segment| {
                [
                    SyntaxKind::AliasExpression,
                    SyntaxKind::ColumnDefinition,
                    SyntaxKind::WithCompoundStatement,
                ]
                .into_iter()
                .any(|it| segment.is_type(it))
            });
            let is_inside_from = parent_stack
                .iter()
                .any(|segment| segment.is_type(SyntaxKind::FromClause));

            match policy {
                "aliases" if is_alias => true,
                "column_aliases" if is_alias => !is_inside_from,
                "table_aliases" if is_alias => is_inside_from,
                _ => false,
            }
        }
    }
}
