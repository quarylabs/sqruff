#[cfg(feature = "python")]
use super::Templater;
use super::TemplaterDocumentation;
#[cfg(feature = "python")]
use super::python::PythonTemplatedFile;
#[cfg(feature = "python")]
use crate::core::config::FluffConfig;
#[cfg(feature = "python")]
use crate::templaters::python_shared::PythonFluffConfig;
#[cfg(feature = "python")]
use crate::templaters::{Formatter, ProcessingMode, TemplaterKind};
#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::{Py, PyAny, Python};
#[cfg(feature = "python")]
use sqruff_lib_core::errors::SQLFluffUserError;
#[cfg(feature = "python")]
use sqruff_lib_core::templaters::TemplatedFile;
#[cfg(feature = "python")]
use std::sync::Arc;

pub struct JinjaTemplater;

#[cfg(feature = "python")]
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

            let py_dict = config.to_python_context(py, TemplaterKind::Jinja).unwrap();
            let python_fluff_config = PythonFluffConfig::from(config);
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
            templated_file.to_templated_file()
        })
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))?;
        Ok(templated_file)
    }

    fn process_single_with_variants(
        &self,
        in_str: &str,
        f_name: &str,
        config: &FluffConfig,
    ) -> Result<Vec<TemplatedFile>, SQLFluffUserError> {
        Python::attach(|py| -> PyResult<Vec<TemplatedFile>> {
            let main_module = PyModule::import(py, "sqruff.templaters.jinja_templater")?;
            let fun: Py<PyAny> = main_module.getattr("process_variants_from_rust")?.into();

            let py_dict = config.to_python_context(py, TemplaterKind::Jinja).unwrap();
            let python_fluff_config = PythonFluffConfig::from(config);
            let returned = fun.call1(
                py,
                (
                    in_str.to_string(),
                    f_name.to_string(),
                    python_fluff_config.to_json_string(),
                    py_dict,
                ),
            )?;
            returned
                .extract::<Vec<PythonTemplatedFile>>(py)?
                .into_iter()
                .map(|templated_file| templated_file.to_templated_file())
                .collect()
        })
        .map_err(|e| SQLFluffUserError::new(format!("Python templater error: {e:?}")))
    }
}

impl TemplaterDocumentation for JinjaTemplater {
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

## Jinja Loader Search Path

`loader_search_path` accepts a comma-separated list of directories for Jinja
[`include`](https://jinja.palletsprojects.com/en/stable/templates/#include) and
[`import`](https://jinja.palletsprojects.com/en/stable/templates/#import)
statements. Locations are relative to the configuration file. For example:

```ini
[sqruff:templater:jinja]
loader_search_path = included_templates,other_templates
```

The configured directories and their subdirectories are available to Jinja.
Given `included_templates/subdir/my_template.sql`, include it relative to its
configured search root:

```jinja
{% include 'subdir/my_template.sql' %}
```

Macros found only through `loader_search_path` are not loaded into the global
namespace. Import them explicitly when needed.

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

When `apply_dbt_builtins` is enabled (the default), common dbt functions like `ref()`, `source()`, and `config()` are available as dummy implementations. This allows linting dbt-style SQL without a full dbt project setup. For full dbt support, use the `dbt` templater instead.

## Library Filters

In addition to variables and macros, the library configured via `library_path` can expose [Jinja filters](https://jinja.palletsprojects.com/en/3.1.x/templates/#filters) to the Jinja environment.

This is achieved by setting a global variable named `SQLFLUFF_JINJA_FILTERS`. `SQLFLUFF_JINJA_FILTERS` is a dictionary where:

- dictionary keys map to the Jinja filter name
- dictionary values map to the Python callable

For example, to make the Airflow filter `ds` available, add the following to the `__init__.py` of the library:

```python
# https://github.com/apache/airflow/blob/main/airflow/templates.py#L50
def ds_filter(value: datetime.date | datetime.time | None) -> str | None:
    """Date filter."""
    if value is None:
        return None
    return value.strftime("%Y-%m-%d")

SQLFLUFF_JINJA_FILTERS = {"ds": ds_filter}
```

Now, `ds` can be used in SQL:

```sql
SELECT "{{ "2000-01-01" | ds }}";
```"#
    }
}

#[cfg(feature = "python")]
impl Templater for JinjaTemplater {
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

    fn process_with_variants(
        &self,
        files: &[(&str, &str)],
        config: &FluffConfig,
        _: &Option<Arc<dyn Formatter>>,
    ) -> Vec<Result<Vec<TemplatedFile>, SQLFluffUserError>> {
        files
            .iter()
            .map(|(content, fname)| self.process_single_with_variants(content, fname, config))
            .collect()
    }
}

#[cfg(all(test, feature = "python"))]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

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
    fn test_jinja_lints_all_render_variants() {
        let source = r#"-- exercise both branches
select 1 AS foo, {% if 1 > 2 %}2 AS boo{% else %}3 AS boo{% endif %}"#;
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = ansi
templater = jinja
rules = CP01
ignore_templated_areas = False
"#,
            None,
        );

        let variants = JinjaTemplater
            .process_with_variants(&[(source, "test.sql")], &config, &None)
            .remove(0)
            .unwrap();
        assert_eq!(variants.len(), 2);

        let linter = Linter::new(config, None, None, false).unwrap();
        let linted = linter
            .lint_string(source, Some("test.sql".to_string()), false)
            .unwrap();
        let positions = linted
            .violations()
            .iter()
            .filter(|violation| violation.rule_code() == "CP01")
            .map(|violation| (violation.line_no, violation.line_pos))
            .collect::<Vec<_>>();

        assert_eq!(positions, vec![(2, 10), (2, 34), (2, 52)]);
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
