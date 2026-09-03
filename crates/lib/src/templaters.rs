use std::sync::Arc;

use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;

use crate::Formatter;
use crate::core::config::FluffConfig;
use crate::templaters::dbt::DBTTemplater;
use crate::templaters::jinja::JinjaTemplater;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::python::PythonTemplater;
use crate::templaters::raw::RawTemplater;

pub mod dbt;
pub mod jinja;
pub mod placeholder;
pub mod python;
#[cfg(feature = "python")]
pub mod python_shared;
pub mod raw;
pub mod types;

pub use types::{PlaceholderStyle, TemplaterKind};

pub static RAW_TEMPLATER: RawTemplater = RawTemplater;
pub static PLACEHOLDER_TEMPLATER: PlaceholderTemplater = PlaceholderTemplater;
pub static PYTHON_TEMPLATER: PythonTemplater = PythonTemplater;
pub static JINJA_TEMPLATER: JinjaTemplater = JinjaTemplater;
pub static DBT_TEMPLATER: DBTTemplater = DBTTemplater;

/// Documentation for every templater, including templaters whose runtime
/// implementation requires the optional Python feature.
pub static TEMPLATER_DOCS: [&'static dyn TemplaterDocumentation; 5] = [
    &RAW_TEMPLATER,
    &PLACEHOLDER_TEMPLATER,
    &PYTHON_TEMPLATER,
    &JINJA_TEMPLATER,
    &DBT_TEMPLATER,
];

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

/// Documentation that is available without compiling a templater's runtime
/// dependencies.
pub trait TemplaterDocumentation: Send + Sync {
    /// The name of the templater.
    fn name(&self) -> &'static str;

    /// Description of the templater.
    fn description(&self) -> &'static str;
}

pub trait Templater: TemplaterDocumentation {
    /// Returns the processing mode for this templater.
    fn processing_mode(&self) -> ProcessingMode;

    /// Process one or more files and return TemplatedFiles.
    ///
    /// Arguments:
    /// - files: Slice of (file_content, file_name) tuples
    /// - config: The configuration to use
    /// - formatter: Optional formatter for output
    ///
    /// Returns a vector of results in the same order as the input files.
    fn process(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        formatter: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<TemplatedFile, SQLFluffUserError>>;

    /// Process files and return every useful rendering of each source file.
    /// Templaters without variant support return their single normal rendering.
    fn process_with_variants(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        formatter: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<Vec<TemplatedFile>, SQLFluffUserError>> {
        self.process(files, config, formatter)
            .into_iter()
            .map(|result| result.map(|templated_file| vec![templated_file]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::TEMPLATER_DOCS;

    #[test]
    fn documentation_lists_every_templater() {
        let names = TEMPLATER_DOCS
            .iter()
            .map(|templater| templater.name())
            .collect::<Vec<_>>();

        assert_eq!(names, ["raw", "placeholder", "python", "jinja", "dbt"]);
    }
}
