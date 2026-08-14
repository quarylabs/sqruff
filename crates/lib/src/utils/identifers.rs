use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::segments::ErasedSegment;

crate::rule_config_enum! {
    /// Which identifiers a rule applies to.
    #[derive(Default)]
    pub enum IdentifiersPolicy {
        /// Every identifier.
        #[default]
        All => "all",
        /// No identifier, i.e. the check is switched off.
        None => "none",
        /// Only aliases.
        Aliases => "aliases",
        /// Only column aliases.
        ColumnAliases => "column_aliases",
        /// Only table aliases.
        TableAliases => "table_aliases",
    }
}

pub fn identifiers_policy_applicable(
    policy: IdentifiersPolicy,
    parent_stack: &[ErasedSegment],
) -> bool {
    match policy {
        IdentifiersPolicy::All => true,
        IdentifiersPolicy::None => false,
        policy => {
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
                IdentifiersPolicy::Aliases if is_alias => true,
                IdentifiersPolicy::ColumnAliases if is_alias => !is_inside_from,
                IdentifiersPolicy::TableAliases if is_alias => is_inside_from,
                _ => false,
            }
        }
    }
}
