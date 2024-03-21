use crate::core::parser::segments::base::Segment;

/// Details about a table alias.
#[derive(Debug, Eq, Hash, Clone)]
pub struct AliasInfo {
    /// Name given to the alias
    pub ref_str: String,
    /// Identifier segment containing the name
    pub segment: Option<Box<dyn Segment>>,
    pub aliased: bool,
    pub from_expression_element: Box<dyn Segment>,
    pub alias_expression: Option<Box<dyn Segment>>,
    pub object_reference: Option<Box<dyn Segment>>,
}

impl PartialEq for AliasInfo {
    fn eq(&self, other: &Self) -> bool {
        self.ref_str == other.ref_str
            && self.segment == other.segment
            && self.aliased == other.aliased
            && &self.from_expression_element == &other.from_expression_element
            && self.alias_expression == other.alias_expression
            && self.object_reference == other.object_reference
    }
}

/// Details about a column alias.
pub struct ColumnAliasInfo {
    pub alias_identifier_name: String,
    pub aliased_segment: Box<dyn Segment>,
    pub column_reference_segments: Vec<Box<dyn Segment>>,
}
