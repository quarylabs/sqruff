use crate::core::parser::segments::base::ErasedSegment;

pub fn identifiers_policy_applicable(policy: &str, parent_stack: &[ErasedSegment]) -> bool {
    match policy {
        "all" => true,
        "none" => false,
        _ => {
            let is_alias = parent_stack.last().map_or(false, |last| {
                ["alias_expression", "column_definition", "with_compound_statement"]
                    .iter()
                    .any(|it| last.is_type(it))
            });
            match policy {
                "aliases" if is_alias => true,
                "column_aliases" if is_alias => {
                    !parent_stack.iter().any(|p| p.is_type("from_clause"))
                }
                _ => false,
            }
        }
    }
}
