use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::templaters::placeholder::PlaceholderTemplater;
use crate::templaters::raw::RawTemplater;

pub mod placeholder;
pub mod raw;

// templaters returns all the templaters that are available in the library
pub fn templaters() -> Vec<Box<dyn Templater>> {
    vec![Box::new(RawTemplater), Box::new(PlaceholderTemplater)]
}

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
        config: Option<&FluffConfig>,
        formatter: Option<&OutputStreamFormatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError>;
}
