use super::python::PythonTemplatedFile;
use super::Templater;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PySlice};
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::base::TemplatedFile;

pub struct JinjaTemplater;

const JINJA_FILE: &str = include_str!("python_templater.py");

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
        config: Option<&crate::core::config::FluffConfig>,
        formatter: Option<&crate::cli::formatters::OutputStreamFormatter>,
    ) -> Result<sqruff_lib_core::templaters::base::TemplatedFile, SQLFluffUserError> {
        let templated_file = Python::with_gil(|py| -> PyResult<TemplatedFile> {
            let fun: Py<PyAny> = PyModule::from_code_bound(py, JINJA_FILE, "", "")?
                .getattr("process_from_rust")?
                .into();

            // pass object with Rust tuple of positional arguments
            let py_dict = PyDict::new_bound(py);
            let args = (in_str.to_string(), f_name.to_string(), py_dict);
            let returned = fun.call1(py, args);

            // Parse the returned value
            let returned = returned?;
            let templated_file: PythonTemplatedFile = returned.extract(py)?;
            Ok(templated_file.to_templated_file())
        })
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {:?}", e)))?;

        Ok(templated_file)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;

    use super::*;

    const JINJA_STRING: &str = "SELECT * FROM {% for c in blah %}{{c}}{% if not loop.last %}, {% endif %}{% endfor %} WHERE {{condition}}\n\n";

    #[test]
    fn test_jinja_templater() {
        let source = r"
    [sqruff]
    templater = jinja
        ";
        let config = FluffConfig::from_source(source);
        let templater = JinjaTemplater;

        let processed = templater
            .process(JINJA_STRING, "test.sql", Some(&config), None)
            .unwrap();

        assert_eq!(
            processed.to_string(),
            "SELECT * FROM f, o, o WHERE a < 10\n\n"
        )
    }
}
