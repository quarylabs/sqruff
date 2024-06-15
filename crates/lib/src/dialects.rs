pub mod ansi;
pub mod ansi_keywords;
pub mod bigquery;
pub mod bigquery_keywords;
pub mod clickhouse;
pub mod clickhouse_keywords;
pub mod postgres;
pub mod postgres_keywords;
pub mod snowflake;
pub mod snowflake_keywords;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SyntaxKind {
    File,
    ColumnReferenceSegment,
    ObjectReference,
    Expression,
    WildcardIdentifier,
    Function,
}

impl SyntaxKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyntaxKind::File => "file",
            SyntaxKind::ColumnReferenceSegment => "column_reference",
            SyntaxKind::ObjectReference => "object_reference",
            SyntaxKind::Expression => "expression",
            SyntaxKind::WildcardIdentifier => "wildcard_identifier",
            SyntaxKind::Function => "function",
        }
    }
}
