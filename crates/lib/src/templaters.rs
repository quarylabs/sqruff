use std::sync::Arc;

use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

use crate::cli::formatters::Formatter;
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

pub trait Templater: Send + Sync {
    /// The name of the templater.
    fn name(&self) -> &'static str;

    /// Description of the templater.
    fn description(&self) -> &'static str;

    /// Process a string and return a TemplatedFile.
    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
        formatter: &Option<Arc<dyn Formatter>>,
    ) -> Result<TemplatedFile, SQLFluffUserError>;
}
