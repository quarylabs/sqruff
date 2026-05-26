use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::templaters::TemplatedFile;

use super::{LintDiagnostic, SkipReason, SourceId};

pub struct ParsedDebugReport {
    pub source_id: SourceId,
    pub tree: Option<ErasedSegment>,
    pub diagnostics: Vec<LintDiagnostic>,
    pub skipped: Option<SkipReason>,
}

pub struct RenderDebugReport {
    pub source_id: SourceId,
    pub templated_file: Option<TemplatedFile>,
    pub diagnostics: Vec<LintDiagnostic>,
    pub skipped: Option<SkipReason>,
}

pub struct LexDebugReport {
    pub source_id: SourceId,
    pub segments: Vec<ErasedSegment>,
    pub diagnostics: Vec<LintDiagnostic>,
    pub skipped: Option<SkipReason>,
}
