use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;

use crate::api::{SkipReason, SourceId, SqruffError};
use crate::core::config::FluffConfig;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::raw::RawTemplater;

#[cfg(feature = "python")]
use crate::templaters::dbt::DBTTemplater;
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

pub static TEMPLATERS: &[TemplaterKind] = TemplaterKind::available();

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

impl From<TemplaterError> for SqruffError {
    fn from(error: TemplaterError) -> Self {
        match error {
            TemplaterError::Failed(error) => Self::Templater(error.value),
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
    ///
    /// Returns a vector of results in the same order as the input files.
    fn process(
        &self,
        files: &[TemplaterInput<'_>],
        config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>>;
}

pub enum TemplaterRuntime {
    Raw(RawTemplater),
    Placeholder(PlaceholderTemplater),

    #[cfg(feature = "python")]
    Python(PythonTemplater),

    #[cfg(feature = "python")]
    Jinja(JinjaTemplater),

    #[cfg(feature = "python")]
    Dbt(DBTTemplater),

    #[cfg(test)]
    Custom(&'static dyn Templater),
}

impl TemplaterRuntime {
    pub fn from_config(config: &FluffConfig) -> Result<Self, SqruffError> {
        let kind = config.templater_kind().map_err(SqruffError::Config)?;
        Ok(Self::from_kind(kind))
    }

    pub fn from_kind(kind: TemplaterKind) -> Self {
        match kind {
            TemplaterKind::Raw => Self::Raw(RawTemplater),
            TemplaterKind::Placeholder => Self::Placeholder(PlaceholderTemplater),
            #[cfg(feature = "python")]
            TemplaterKind::Python => Self::Python(PythonTemplater),
            #[cfg(feature = "python")]
            TemplaterKind::Jinja => Self::Jinja(JinjaTemplater),
            #[cfg(feature = "python")]
            TemplaterKind::Dbt => Self::Dbt(DBTTemplater),
        }
    }

    #[cfg(test)]
    pub(crate) fn custom(templater: &'static dyn Templater) -> Self {
        Self::Custom(templater)
    }

    pub fn name(&self) -> &'static str {
        <Self as Templater>::name(self)
    }

    pub fn description(&self) -> &'static str {
        <Self as Templater>::description(self)
    }

    pub fn processing_mode(&self) -> ProcessingMode {
        <Self as Templater>::processing_mode(self)
    }

    pub fn process(
        &self,
        files: &[TemplaterInput<'_>],
        config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
        <Self as Templater>::process(self, files, config)
    }
}

impl Templater for TemplaterRuntime {
    fn name(&self) -> &'static str {
        match self {
            Self::Raw(t) => t.name(),
            Self::Placeholder(t) => t.name(),
            #[cfg(feature = "python")]
            Self::Python(t) => t.name(),
            #[cfg(feature = "python")]
            Self::Jinja(t) => t.name(),
            #[cfg(feature = "python")]
            Self::Dbt(t) => t.name(),
            #[cfg(test)]
            Self::Custom(t) => t.name(),
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Raw(t) => t.description(),
            Self::Placeholder(t) => t.description(),
            #[cfg(feature = "python")]
            Self::Python(t) => t.description(),
            #[cfg(feature = "python")]
            Self::Jinja(t) => t.description(),
            #[cfg(feature = "python")]
            Self::Dbt(t) => t.description(),
            #[cfg(test)]
            Self::Custom(t) => t.description(),
        }
    }

    fn processing_mode(&self) -> ProcessingMode {
        match self {
            Self::Raw(t) => t.processing_mode(),
            Self::Placeholder(t) => t.processing_mode(),
            #[cfg(feature = "python")]
            Self::Python(t) => t.processing_mode(),
            #[cfg(feature = "python")]
            Self::Jinja(t) => t.processing_mode(),
            #[cfg(feature = "python")]
            Self::Dbt(t) => t.processing_mode(),
            #[cfg(test)]
            Self::Custom(t) => t.processing_mode(),
        }
    }

    fn process(
        &self,
        files: &[TemplaterInput<'_>],
        config: &FluffConfig,
    ) -> Vec<Result<TemplaterOutput, TemplaterError>> {
        match self {
            Self::Raw(t) => t.process(files, config),
            Self::Placeholder(t) => t.process(files, config),
            #[cfg(feature = "python")]
            Self::Python(t) => t.process(files, config),
            #[cfg(feature = "python")]
            Self::Jinja(t) => t.process(files, config),
            #[cfg(feature = "python")]
            Self::Dbt(t) => t.process(files, config),
            #[cfg(test)]
            Self::Custom(t) => t.process(files, config),
        }
    }
}

pub(crate) fn source_id_name(source_id: &SourceId) -> String {
    match source_id {
        SourceId::Stdin => "<stdin>".to_string(),
        SourceId::Path(path) => path.to_string_lossy().into_owned(),
        SourceId::Virtual(name) => name.clone(),
    }
}
