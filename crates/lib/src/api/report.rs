use super::{LintDiagnostic, SourceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub files: Vec<FileReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    pub source_id: SourceId,
    pub diagnostics: Vec<LintDiagnostic>,
    pub fixed_source: Option<String>,
    pub skipped: Option<SkipReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipReason {
    pub message: String,
}
