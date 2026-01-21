use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::IdentifiersPolicy;

pub fn identifiers_policy_applicable(
    policy: IdentifiersPolicy,
    parent_stack: &[ErasedSegment],
) -> bool {
    match policy {
        IdentifiersPolicy::All => true,
        IdentifiersPolicy::None => false,
        IdentifiersPolicy::Aliases | IdentifiersPolicy::ColumnAliases => {
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

            if !is_alias {
                return false;
            }

            match policy {
                IdentifiersPolicy::Aliases => true,
                IdentifiersPolicy::ColumnAliases => !is_inside_from,
                _ => false,
            }
        }
    }
}
