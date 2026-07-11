use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub message: String,
    pub code: Option<String>,
    pub line: usize,
    pub column: usize,
    pub source_range: Range<usize>,
    pub fixable: bool,
}
