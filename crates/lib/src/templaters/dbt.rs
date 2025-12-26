use super::Templater;
use super::python::PythonTemplatedFile;
use crate::core::config::FluffConfig;
use crate::templaters::Formatter;
use crate::templaters::python_shared::PythonFluffConfig;
use pyo3::prelude::*;
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;
use std::sync::Arc;

pub struct DBTTemplater;

impl Templater for DBTTemplater {
    fn name(&self) -> &'static str {
        "dbt"
    }

    fn can_process_in_parallel(&self) -> bool {
        false
    }

    fn description(&self) -> &'static str {
        "Not fully implemented yet. More details to come."
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
        _: &Option<Arc<dyn Formatter>>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let templated_file = Python::attach(|py| -> PyResult<TemplatedFile> {
            let main_module = PyModule::import(py, "sqruff.templaters.dbt_templater")?;
            let fun: Py<PyAny> = main_module.getattr("process_from_rust")?.into();

            let py_dict = config.to_python_context(py, "dbt").unwrap();
            let python_fluff_config: PythonFluffConfig = config.clone().into();
            let args = (
                in_str.to_string(),
                f_name.to_string(),
                python_fluff_config.to_json_string(),
                py_dict,
            );
            let returned = fun.call1(py, args);

            let returned = returned?;
            let templated_file: PythonTemplatedFile = returned.extract(py)?;
            Ok(templated_file.to_templated_file())
        })
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))?;
        Ok(templated_file)
    }
}
