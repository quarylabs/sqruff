use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

pub fn identifiers_policy_applicable(policy: &str, parent_stack: &[ErasedSegment]) -> bool {
    match policy {
        "all" => true,
        "none" => false,
        _ => {
            let is_alias = parent_stack.last().is_some_and(|last| {
                [
                    SyntaxKind::AliasExpression,
                    SyntaxKind::ColumnDefinition,
                    SyntaxKind::WithCompoundStatement,
                ]
                .into_iter()
                .any(|it| last.is_type(it))
            });

            match policy {
                "aliases" if is_alias => true,
                "column_aliases" if is_alias => !parent_stack
                    .iter()
                    .any(|p| p.is_type(SyntaxKind::FromClause)),
                _ => false,
            }
        }
    }
}
