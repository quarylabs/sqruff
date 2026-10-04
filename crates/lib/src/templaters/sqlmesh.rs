#[cfg(feature = "python")]
use std::sync::Arc;

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyList;
#[cfg(feature = "python")]
use sqruff_lib_core::errors::SQLFluffUserError;
#[cfg(feature = "python")]
use sqruff_lib_core::templaters::TemplatedFile;

use super::TemplaterDocumentation;
#[cfg(feature = "python")]
use super::python::PythonTemplatedFile;
#[cfg(feature = "python")]
use super::python_shared::PythonFluffConfig;
#[cfg(feature = "python")]
use super::{ProcessingMode, Templater};
#[cfg(feature = "python")]
use crate::Formatter;
#[cfg(feature = "python")]
use crate::core::config::FluffConfig;

pub struct SQLMeshTemplater;

impl TemplaterDocumentation for SQLMeshTemplater {
    fn name(&self) -> &'static str {
        "sqlmesh"
    }

    fn description(&self) -> &'static str {
        r#"The SQLMesh templater processes SQLMesh model files using their project context. Install the Python-enabled sqruff package and SQLMesh with `pip install 'sqruff[sqlmesh]'`.

Enable it with `templater = sqlmesh` in the `[sqruff]` section. Configure the optional `project_dir`, `config`, and `gateway` keys under `[sqruff:templater:sqlmesh]`; `SQLMESH_PROJECT_DIR` or the working directory is used when `project_dir` is unset.

For plain SQL models, sqruff strips the `MODEL (...)` header and lints the remaining SQL with exact source positions without loading SQLMesh. Inline macros are substituted while preserving literal source regions where possible. Structural macros and Jinja queries are rendered through SQLMesh and mapped conservatively as templated regions."#
    }
}

#[cfg(feature = "python")]
impl Templater for SQLMeshTemplater {
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

        let batch = Python::attach(
            |py| -> PyResult<Vec<(Option<PythonTemplatedFile>, Option<String>)>> {
                let module = PyModule::import(py, "sqruff.templaters.sqlmesh_templater")?;
                let process = module.getattr("process_batch_from_rust")?;
                let py_files: Vec<(String, String)> = files
                    .iter()
                    .map(|(content, fname)| (content.to_string(), fname.to_string()))
                    .collect();
                let py_files = PyList::new(py, py_files)?;
                let config = PythonFluffConfig::from(config).to_json_string();
                process.call1((py_files, config))?.extract()
            },
        );

        match batch {
            Ok(results) if results.len() == files.len() => results
                .into_iter()
                .map(|(file, error)| {
                    if let Some(error) = error {
                        Err(SQLFluffUserError::new(error))
                    } else if let Some(file) = file {
                        file.to_templated_file().map_err(|error| {
                            SQLFluffUserError::new(format!(
                                "Failed to convert SQLMesh templated file: {error:?}"
                            ))
                        })
                    } else {
                        Err(SQLFluffUserError::new(
                            "SQLMesh returned neither a templated file nor an error".to_string(),
                        ))
                    }
                })
                .collect(),
            Ok(results) => files
                .iter()
                .map(|_| {
                    Err(SQLFluffUserError::new(format!(
                        "SQLMesh returned {} results for {} files",
                        results.len(),
                        files.len()
                    )))
                })
                .collect(),
            Err(error) => files
                .iter()
                .map(|_| {
                    Err(SQLFluffUserError::new(format!(
                        "Python SQLMesh templater error: {error:?}"
                    )))
                })
                .collect(),
        }
    }
}
