use super::Templater;
use super::python::PythonTemplatedFile;
use crate::core::config::FluffConfig;
use crate::templaters::python_shared::PythonFluffConfig;
use crate::templaters::{Formatter, ProcessingMode};
use pyo3::prelude::*;
use pyo3::{Py, PyAny, Python};
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::templaters::TemplatedFile;
use std::sync::Arc;

pub struct JinjaTemplater;

impl JinjaTemplater {
    fn process_single(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        let templated_file = Python::attach(|py| -> PyResult<TemplatedFile> {
            let main_module = PyModule::import(py, "sqruff.templaters.jinja_templater")?;
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
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))?;
        Ok(templated_file)
    }
}

impl Templater for JinjaTemplater {
    fn name(&self) -> &'static str {
        "jinja"
    }

    fn description(&self) -> &'static str {
        r#"The jinja templater uses the Jinja2 templating engine to process SQL files with dynamic content. This is useful for SQL that uses variables, loops, conditionals, and macros.

**Note:** This templater requires Python and the sqruff Python package. Install it with:

```bash
pip install sqruff
```

Alternatively, build sqruff from source with the `python` feature enabled.

## Activation

Enable the jinja templater in your `.sqruff` config file:

```ini
[sqruff]
templater = jinja
```

## Configuration Options

Configuration options are set in the `[sqruff:templater:jinja]` section:

```ini
[sqruff:templater:jinja]
# Apply dbt builtins (ref, source, config, etc.) - enabled by default
apply_dbt_builtins = True

# Paths to load macros from (comma-separated list of directories/files)
load_macros_from_path = ./macros

# Paths for Jinja2 FileSystemLoader to search for templates
loader_search_path = ./templates

# Path to a Python library to make available in the Jinja environment
library_path = ./my_library

# Set to True to ignore templating errors (useful for partial linting)
ignore_templating = False
```

## Template Variables (Context)

Define template variables in the `[sqruff:templater:jinja:context]` section:

```ini
[sqruff:templater:jinja:context]
my_variable = some_value
table_name = users
environment = production
```

These variables can then be used in your SQL files:

```sql
SELECT * FROM {{ table_name }}
WHERE environment = '{{ environment }}'
```

## Example

Given the following SQL file with Jinja templating:

```sql
{% set columns = ['id', 'name', 'email'] %}

SELECT
    {% for col in columns %}
    {{ col }}{% if not loop.last %},{% endif %}
    {% endfor %}
FROM users
```

The jinja templater will expand this to valid SQL before linting.

## dbt Builtins

When `apply_dbt_builtins` is enabled (the default), common dbt functions like `ref()`, `source()`, and `config()` are available as dummy implementations. This allows linting dbt-style SQL without a full dbt project setup. For full dbt support, use the `dbt` templater instead."#
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::Sequential
    }

    fn process(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        _: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<TemplatedFile, SQLFluffUserError>> {
        files
            .iter()
            .map(|(content, fname)| self.process_single(content, fname, config))
            .collect()
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

        let results = templater.process(&[(JINJA_STRING, "test.sql")], &config, &None);
        let processed = results.into_iter().next().unwrap().unwrap();

        assert_eq!(
            processed.templated(),
            "\n\n\nSELECT\n    event_id\n    \n    , campaign\n    \n    , click_item\n    \nFROM events\n"
        )
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
        let results = templater.process(&[(instr, "test.sql")], &config, &None);
        let processed = results.into_iter().next().unwrap().unwrap();

        assert_eq!(processed.templated(), "\n    \n    SELECT 1\n\n");
    }
}
