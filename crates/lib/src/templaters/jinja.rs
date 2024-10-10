use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::templaters::Templater;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

#[derive(Default)]
pub struct JinjaTemplater;

impl Templater for JinjaTemplater {
    fn name(&self) -> &'static str {
        "jinja"
    }

    fn description(&self) -> &'static str {
        todo!()
        // "Describe where the macro paths is in config"
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: Option<&FluffConfig>,
        formatter: Option<&OutputStreamFormatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let config = config.ok_or(SQLFluffUserError::new(
            "Jinja templating requires a config".to_string(),
        ))?;

        let py_util = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/templaters/python/utils/utils.py"
        ));
        let py_app = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/templaters/python/app.py"));

        let from_python = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            PyModule::from_code_bound(py, py_util, "utils.utils", "utils.utils")?;
            let app: Py<PyAny> = PyModule::from_code_bound(py, py_app, "", "")?
                .getattr("concat_wrapper")?
                .into();
            app.call1(py, ("test", "test"))
        }).unwrap();

        let output = from_python.to_string();

        println!("{}", output);

        todo!()
    }
}
