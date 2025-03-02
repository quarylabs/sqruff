use super::Templater;
use super::python::PythonTemplatedFile;
use crate::core::config::FluffConfig;
use crate::templaters::Formatter;
use crate::templaters::python_shared::PythonFluffConfig;
use crate::templaters::python_shared::add_temp_files_to_site_packages;
use crate::templaters::python_shared::add_venv_site_packages;
use pyo3::prelude::*;
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;
use std::sync::Arc;

pub struct DBTTemplater;

impl Templater for DBTTemplater {
    fn name(&self) -> &'static str {
        "dbt"
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
        let templated_file = Python::with_gil(|py| -> PyResult<TemplatedFile> {
            let files = [
                (
                    "sqruff_templaters/__init__.py",
                    include_str!("sqruff_templaters/__init__.py"),
                ),
                (
                    "sqruff_templaters/dbt_templater.py",
                    include_str!("sqruff_templaters/dbt_templater.py"),
                ),
                (
                    "sqruff_templaters/jinja_templater.py",
                    include_str!("sqruff_templaters/jinja_templater.py"),
                ),
                (
                    "sqruff_templaters/jinja_templater_builtins_common.py",
                    include_str!("sqruff_templaters/jinja_templater_builtins_common.py"),
                ),
                (
                    "sqruff_templaters/jinja_templater_builtins_dbt.py",
                    include_str!("sqruff_templaters/jinja_templater_builtins_dbt.py"),
                ),
                (
                    "sqruff_templaters/jinja_templater_tracers.py",
                    include_str!("sqruff_templaters/jinja_templater_tracers.py"),
                ),
                (
                    "sqruff_templaters/python_templater.py",
                    include_str!("sqruff_templaters/python_templater.py"),
                ),
            ];

            add_venv_site_packages(py)?;
            add_temp_files_to_site_packages(py, &files)?;

            let main_module = PyModule::import(py, "sqruff_templaters.dbt_templater")?;
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
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {:?}", e)))?;
        Ok(templated_file)
    }
}
