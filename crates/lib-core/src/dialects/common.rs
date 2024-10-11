use smol_str::SmolStr;

use crate::parser::segments::base::ErasedSegment;

/// Details about a table alias.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct AliasInfo {
    /// Name given to the alias
    pub ref_str: SmolStr,
    /// Identifier segment containing the name
    pub segment: Option<ErasedSegment>,
    pub aliased: bool,
    pub from_expression_element: ErasedSegment,
    pub alias_expression: Option<ErasedSegment>,
    pub object_reference: Option<ErasedSegment>,
}

/// Details about a column alias.
#[derive(Clone, Debug)]
pub struct ColumnAliasInfo {
    pub alias_identifier_name: SmolStr,
    pub aliased_segment: ErasedSegment,
    pub column_reference_segments: Vec<ErasedSegment>,
}
