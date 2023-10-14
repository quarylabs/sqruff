use crate::core::parser::segments::base::Segment;

/// Details about a table alias.
pub struct AliasInfo {
    /// Name given to the alias
    ref_str: String,
    /// Identifier segment containing the name
    segment: Option<Box<dyn Segment>>,
    aliased: bool,
    from_expression_element: Box<dyn Segment>,
    alias_expression: Option<Box<dyn Segment>>,
    object_reference: Option<Box<dyn Segment>>,
}

/// Details about a column alias.
pub struct ColumnAliasInfo {
    alias_identifier_name: String,
    aliased_segment: Box<dyn Segment>,
    column_reference_segments: Vec<Box<dyn Segment>>,
}
