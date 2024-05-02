use crate::core::parser::segments::base::ErasedSegment;

/// Details about a table alias.
#[derive(Debug, Eq, Hash, Clone)]
#[allow(clippy::field_reassign_with_default, clippy::derived_hash_with_manual_eq)]
pub struct AliasInfo {
    /// Name given to the alias
    pub ref_str: String,
    /// Identifier segment containing the name
    pub segment: Option<ErasedSegment>,
    pub aliased: bool,
    pub from_expression_element: ErasedSegment,
    pub alias_expression: Option<ErasedSegment>,
    pub object_reference: Option<ErasedSegment>,
}

impl PartialEq for AliasInfo {
    fn eq(&self, other: &Self) -> bool {
        self.ref_str == other.ref_str
            && self.segment == other.segment
            && self.aliased == other.aliased
            && self.from_expression_element == other.from_expression_element
            && self.alias_expression == other.alias_expression
            && self.object_reference == other.object_reference
    }
}

/// Details about a column alias.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ColumnAliasInfo {
    pub alias_identifier_name: String,
    pub aliased_segment: ErasedSegment,
    pub column_reference_segments: Vec<ErasedSegment>,
}
