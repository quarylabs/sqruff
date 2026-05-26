use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;

use crate::api::{SkipReason, SourceId};
use crate::core::config::FluffConfig;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::raw::RawTemplater;

#[cfg(feature = "python")]
use crate::templaters::jinja::JinjaTemplater;
#[cfg(feature = "python")]
use crate::templaters::python::PythonTemplater;

#[cfg(feature = "python")]
pub mod dbt;
#[cfg(feature = "python")]
pub mod jinja;
pub mod placeholder;
#[cfg(feature = "python")]
pub mod python;
#[cfg(feature = "python")]
pub mod python_shared;
pub mod raw;
pub mod types;

pub use types::{PlaceholderStyle, TemplaterKind};

pub static RAW_TEMPLATER: RawTemplater = RawTemplater;
pub static PLACEHOLDER_TEMPLATER: PlaceholderTemplater = PlaceholderTemplater;
#[cfg(feature = "python")]
pub static PYTHON_TEMPLATER: PythonTemplater = PythonTemplater;
#[cfg(feature = "python")]
pub static JINJA_TEMPLATER: JinjaTemplater = JinjaTemplater;
#[cfg(feature = "python")]
pub static DBT_TEMPLATER: dbt::DBTTemplater = dbt::DBTTemplater;

// templaters returns all the templaters that are available in the library
#[cfg(feature = "python")]
pub static TEMPLATERS: [&'static dyn Templater; 5] = [
    &RAW_TEMPLATER,
    &PLACEHOLDER_TEMPLATER,
    &PYTHON_TEMPLATER,
    &JINJA_TEMPLATER,
    &DBT_TEMPLATER,
];

#[cfg(not(feature = "python"))]
pub static TEMPLATERS: [&'static dyn Templater; 2] = [&RAW_TEMPLATER, &PLACEHOLDER_TEMPLATER];

/// How a templater processes files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    /// Files can be processed individually and in parallel using Rayon.
    /// Used by simple templaters like raw and placeholder.
    Parallel,
    /// Files must be processed sequentially, one at a time.
    /// Used by templaters that have Python GIL restrictions.
    Sequential,
    /// Files benefit from batch processing with shared state.
    /// The templater will receive all files at once and can optimize initialization.
    /// Used by dbt to share manifest loading across files.
    Batch,
}

pub struct TemplaterInput<'a> {
    pub source: &'a str,
    pub source_id: &'a SourceId,
}

pub enum TemplaterOutput {
    Rendered(TemplatedFile),
    Skipped(SkipReason),
}

#[derive(Debug)]
pub enum TemplaterError {
    Failed(SQLFluffUserError),
}

impl From<SQLFluffUserError> for TemplaterError {
    fn from(error: SQLFluffUserError) -> Self {
        Self::Failed(error)
    }
}

impl TemplaterError {
    pub fn into_user_error(self) -> SQLFluffUserError {
        match self {
            Self::Failed(error) => error,
        }
    }
}

pub trait Templater: Send + Sync {
    /// The name of the templater.
    fn name(&self) -> &'static str;

    /// Description of the templater.
    fn description(&self) -> &'static str;

    /// Returns the processing mode for this templater.
    fn processing_mode(&self) -> ProcessingMode;

    /// Process one or more files and return typed templater outcomes.
    ///
    /// Arguments:
    /// - files: Input files with source text and identity.
    /// - config: The configuration to use
    /// Returns a vector of results in the same order as the input files.
    fn process(
        &self,
        files: &[TemplaterInput<'_>],
        config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>>;
}

pub(crate) fn source_id_name(source_id: &SourceId) -> String {
    match source_id {
        SourceId::Stdin => "<stdin>".to_string(),
        SourceId::Path(path) => path.to_string_lossy().into_owned(),
        SourceId::Virtual(name) => name.clone(),
    }
}
