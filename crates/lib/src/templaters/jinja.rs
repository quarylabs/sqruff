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

pub struct JinjaTemplater;

impl Templater for JinjaTemplater {
    fn name(&self) -> &'static str {
        "jinja"
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

            let main_module = PyModule::import(py, "sqruff_templaters.jinja_templater")?;
            let fun: Py<PyAny> = main_module.getattr("process_from_rust")?.into();

            let py_dict = config.to_python_context(py, "jinja").unwrap();
            let python_fluff_config: PythonFluffConfig = config.clone().into();
            let args = (
                in_str.to_string(),
                f_name.to_string(),
                python_fluff_config.to_json_string(),
                py_dict,
            );
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

    const JINJA_STRING: &str = "
{% set event_columns = ['campaign', 'click_item'] %}

SELECT
    event_id
    {% for event_column in event_columns %}
    , {{ event_column }}
    {% endfor %}
FROM events
";

    #[test]
    fn test_jinja_templater() {
        let source = r"
    [sqruff]
    templater = jinja
        ";
        let config = FluffConfig::from_source(source, None);
        let templater = JinjaTemplater;

        let processed = templater
            .process(JINJA_STRING, "test.sql", &config, &None)
            .unwrap();

        assert_eq!(
            processed.templated(),
            "\n\n\nSELECT\n    event_id\n    \n    , campaign\n    \n    , click_item\n    \nFROM events\n"
        )
    }

    #[test]
    fn test_jinja_templater_slice_file_empty() {
        let source = r"
    [sqruff]
    templater = jinja
        ";
        let config = FluffConfig::from_source(source, None);
        let templater = JinjaTemplater;

        let processed = templater
            .process("", "test.sql", &config, &None)
            .unwrap();

        // For an empty file, we expect no slices
        assert_eq!(processed.sliced_file.len(), 0);
    }

    #[test]
    fn test_jinja_templater_slice_file_simple() {
        let source = r"
[sqruff]
templater = jinja
        ";
        let config = FluffConfig::from_source(source, None);
        let templater = JinjaTemplater;

        let processed = templater
            .process("foo", "test.sql", &config, &None)
            .unwrap();

        // For a simple literal string, we expect one slice of type "literal"
        let slices = processed.sliced_file.clone();
        assert_eq!(slices.len(), 1);
        
        let slice = &slices[0];
        assert_eq!(slice.slice_type, "literal");
        assert_eq!(slice.source_slice, 0..3);
        assert_eq!(slice.templated_slice, 0..3);
    }

    #[test]
    fn test_jinja_templater_slice_file_with_comment_and_variable() {
        let source = r"
[sqruff]
templater = jinja

[sqruff.templater.jinja.context]
blah = bar
        ";
        let config = FluffConfig::from_source(source, None);
        let templater = JinjaTemplater;

        let processed = templater
            .process("SELECT {{blah}}, boo {# comment #} from something", "test.sql", &config, &None)
            .unwrap();

        // Check the slices match the expected pattern
        let slices = processed.sliced_file.clone();
        assert_eq!(slices.len(), 5);
        
        // Check each slice type and range
        assert_eq!(slices[0].slice_type, "literal");
        assert_eq!(slices[0].source_slice, 0..7);
        assert_eq!(slices[0].templated_slice, 0..7);
        
        assert_eq!(slices[1].slice_type, "templated");
        assert_eq!(slices[1].source_slice, 7..15);
        assert_eq!(slices[1].templated_slice, 7..13);
        
        assert_eq!(slices[2].slice_type, "literal");
        assert_eq!(slices[2].source_slice, 15..21);
        assert_eq!(slices[2].templated_slice, 13..19);
        
        assert_eq!(slices[3].slice_type, "comment");
        assert_eq!(slices[3].source_slice, 21..34);
        assert_eq!(slices[3].templated_slice, 19..19);
        
        assert_eq!(slices[4].slice_type, "literal");
        assert_eq!(slices[4].source_slice, 34..49);
        assert_eq!(slices[4].templated_slice, 19..34);
        
        // Verify the templated output
        assert_eq!(processed.templated(), "SELECT foobar, boo  from something");
    }


    #[test]
    fn test_jinja_templater_dynamic_variable_no_violations() {
        let source = r"
    [sqruff]
    templater = jinja
        ";
        let config = FluffConfig::from_source(source, None);
        let templater = JinjaTemplater;
        let instr = r#"{% if True %}
    {% set some_var %}1{% endset %}
    SELECT {{some_var}}
{% endif %}
"#;
        let processed = templater
            .process(instr, "test.sql", &config, &None)
            .unwrap();

        assert_eq!(processed.templated(), "\n    \n    SELECT 1\n\n");
    }
}
