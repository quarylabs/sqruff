use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use pyo3::ffi::c_str;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;
use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::templaters::Templater;

pub struct DBTTemplater {}

impl Templater for DBTTemplater {
    fn name(&self) -> &'static str {
        "dbt"
    }

    fn description(&self) -> &'static str {
        todo!()
    }

    fn template_selection(&self) -> &str {
        todo!()
    }

    fn config_pairs(&self) -> (String, String) {
        todo!()
    }

    fn sequence_files(&self, f_names: Vec<String>, config: Option<&FluffConfig>, formatter: Option<&OutputStreamFormatter>) -> Vec<String> {
        todo!()
    }

    fn process(&self, in_str: &str, f_name: &str, config: Option<&FluffConfig>, formatter: Option<&OutputStreamFormatter>) -> Result<TemplatedFile, SQLFluffUserError> {
        todo!()
    }
}