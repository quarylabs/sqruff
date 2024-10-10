use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::templaters::Templater;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

pub struct JinjaTemplater {}

impl Templater for JinjaTemplater {
    fn name(&self) -> &'static str {
        "jinja"
    }

    fn description(&self) -> &'static str {
        todo!()
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: Option<&FluffConfig>,
        formatter: Option<&OutputStreamFormatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        todo!()
    }
}
