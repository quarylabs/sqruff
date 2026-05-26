pub(crate) mod github;
pub(crate) mod human;
pub(crate) mod json;

use sqruff_lib::api::{RunReport, SourceId};
use sqruff_lib::core::config::FluffConfig;

use crate::commands::Format;
use crate::reporters::github::GithubReporter;
use crate::reporters::human::HumanReporter;
use crate::reporters::json::JsonReporter;

pub(crate) type CliError = Box<dyn std::error::Error + Send + Sync>;

pub(crate) enum Reporter {
    Human(HumanReporter),
    Json(JsonReporter),
    Github(GithubReporter),
}

impl Reporter {
    pub(crate) fn new(format: Format, config: &FluffConfig) -> Self {
        match format {
            Format::Human => Self::Human(HumanReporter::new(config)),
            Format::GithubAnnotationNative => Self::Github(GithubReporter::new()),
            Format::Json => Self::Json(JsonReporter::default()),
        }
    }

    pub(crate) fn emit(&mut self, report: &RunReport) -> Result<(), CliError> {
        match self {
            Self::Human(r) => r.emit(report),
            Self::Json(r) => r.emit(report),
            Self::Github(r) => r.emit(report),
        }
    }

    pub(crate) fn emit_diagnostics(&mut self, report: &RunReport) -> Result<(), CliError> {
        match self {
            Self::Human(r) => r.emit_diagnostics(report),
            Self::Json(r) => r.emit(report),
            Self::Github(r) => r.emit(report),
        }
    }

    pub(crate) fn emit_no_changes(&mut self, report: &RunReport) -> Result<(), CliError> {
        match self {
            Self::Human(r) => r.emit_no_changes(report),
            Self::Json(r) => r.emit(report),
            Self::Github(r) => r.emit(report),
        }
    }
}

pub(crate) fn display_source_id(source_id: &SourceId) -> String {
    match source_id {
        SourceId::Stdin => "<string>".into(),
        SourceId::Path(path) => path.to_string_lossy().into_owned(),
        SourceId::Virtual(name) => name.clone(),
    }
}
