use super::Templater;
use super::python::PythonTemplatedFile;
use crate::core::config::FluffConfig;
use crate::templaters::python_shared::PythonFluffConfig;
use crate::templaters::{Formatter, ProcessingMode};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;
use std::sync::Arc;

pub struct DBTTemplater;

impl Templater for DBTTemplater {
    fn name(&self) -> &'static str {
        "dbt"
    }

    fn description(&self) -> &'static str {
        "dbt templater for processing dbt models with Jinja templating and manifest support."
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::Batch
    }

    fn process(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        _: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<TemplatedFile, SQLFluffUserError>> {
        if files.is_empty() {
            return Vec::new();
        }

        Python::attach(|py| -> Vec<Result<TemplatedFile, SQLFluffUserError>> {
            let main_module = match PyModule::import(py, "sqruff.templaters.dbt_templater") {
                Ok(m) => m,
                Err(e) => {
                    return files
                        .iter()
                        .map(|_| {
                            Err(SQLFluffUserError::new(format!(
                                "Failed to import dbt_templater module: {e:?}"
                            )))
                        })
                        .collect();
                }
            };

            let fun: Py<PyAny> = match main_module.getattr("process_batch_from_rust") {
                Ok(f) => f.into(),
                Err(e) => {
                    return files
                        .iter()
                        .map(|_| {
                            Err(SQLFluffUserError::new(format!(
                                "Failed to get process_batch_from_rust function: {e:?}"
                            )))
                        })
                        .collect();
                }
            };

            let py_dict = match config.to_python_context(py, "dbt") {
                Ok(d) => d,
                Err(e) => {
                    return files
                        .iter()
                        .map(|_| {
                            Err(SQLFluffUserError::new(format!(
                                "Failed to create Python context: {e:?}"
                            )))
                        })
                        .collect();
                }
            };

            let python_fluff_config: PythonFluffConfig = config.into();

            // Convert files to Python list of tuples
            let py_files: Vec<(String, String)> = files
                .iter()
                .map(|(content, fname)| (content.to_string(), fname.to_string()))
                .collect();

            let py_files_list = PyList::new(py, &py_files).unwrap();

            let args = (py_files_list, python_fluff_config.to_json_string(), py_dict);

            match fun.call1(py, args) {
                Ok(returned) => {
                    // The Python function returns a list of (TemplatedFile | None, error_message | None)
                    let results: Vec<(Option<PythonTemplatedFile>, Option<String>)> =
                        match returned.extract(py) {
                            Ok(r) => r,
                            Err(e) => {
                                return files
                                    .iter()
                                    .map(|_| {
                                        Err(SQLFluffUserError::new(format!(
                                            "Failed to extract batch results: {e:?}"
                                        )))
                                    })
                                    .collect();
                            }
                        };

                    results
                        .into_iter()
                        .map(|(templated_file, error)| {
                            if let Some(err_msg) = error {
                                Err(SQLFluffUserError::new(err_msg))
                            } else if let Some(tf) = templated_file {
                                Ok(tf.to_templated_file())
                            } else {
                                Err(SQLFluffUserError::new(
                                    "No templated file or error returned".to_string(),
                                ))
                            }
                        })
                        .collect()
                }
                Err(e) => files
                    .iter()
                    .map(|_| {
                        Err(SQLFluffUserError::new(format!(
                            "Python batch templater error: {e:?}"
                        )))
                    })
                    .collect(),
            }
        })
    }
}
