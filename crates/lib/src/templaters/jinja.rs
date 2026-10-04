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

# Paths beneath the macro load paths to exclude
exclude_macros_from_path = ./macros/excluded

# Paths for Jinja2 FileSystemLoader to search for templates
loader_search_path = ./templates

# Path to a Python library to make available in the Jinja environment
library_path = ./my_library

# Set to True to ignore templating errors (useful for partial linting)
ignore_templating = False
```

## Jinja Macro Paths

`load_macros_from_path` loads macros from the configured files and directories,
including their subdirectories, into the Jinja global namespace.
`exclude_macros_from_path` accepts paths in the same format and skips matching
macro files. This is useful when a macro tree contains custom Jinja tags that
sqruff should not parse.

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
    fn test_jinja_literal_fast_path_through_rust() {
        let config = FluffConfig::from_source("[sqruff]\ntemplater = jinja\n", None);
        let source = "SELECT 'π'\n";
        let variants = JinjaTemplater
            .process_with_variants(&[(source, "test.sql")], &config, &None)
            .remove(0)
            .unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].templated(), source);
        assert_eq!(variants[0].sliced_file.len(), 1);
        assert_eq!(variants[0].sliced_file[0].source_slice, 0..source.len());
        assert_eq!(variants[0].raw_sliced().len(), 1);
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
    fn test_jinja_variant_limit_controls_branch_linting() {
        let fixture =
            std::fs::read_to_string("test/fixtures/linter/jinja_variants/branching_cp01.sql")
                .unwrap();
        let source = fixture.lines().skip(1).collect::<Vec<_>>().join("\n");
        let config = |limit| {
            FluffConfig::from_source(
                &format!(
                    "[sqruff]\ndialect = ansi\ntemplater = jinja\nrules = CP01\nrender_variant_limit = {limit}\n\
                     [sqruff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n"
                ),
                None,
            )
        };
        let lint_positions = |limit| {
            Linter::new(config(limit), None, None, false)
                .unwrap()
                .lint_string(&source, Some("branches.sql".to_string()), false)
                .unwrap()
                .violations()
                .iter()
                .filter(|violation| violation.rule_code() == "CP01")
                .map(|violation| (violation.line_no, violation.line_pos))
                .collect::<Vec<_>>()
        };

        let one_branch = lint_positions(1);
        let five_branches = lint_positions(5);
        assert!(
            five_branches.len() > one_branch.len(),
            "one={one_branch:?}, five={five_branches:?}"
        );
    }

    #[test]
    fn test_jinja_alternate_parse_errors_do_not_invalidate_root() {
        let source =
            std::fs::read_to_string("test/fixtures/linter/jinja_variants/branching_cp01.sql")
                .unwrap();
        let config = FluffConfig::from_source(
            "[sqruff]\ndialect = ansi\ntemplater = jinja\nrules = CP01\nrender_variant_limit = 5\n",
            None,
        );
        let linter = Linter::new(config, None, None, false).unwrap();
        let tables = sqruff_lib_core::parser::segments::Tables::default();
        let mut parsed = linter
            .parse_string(&tables, &source, Some("branches.sql".to_string()))
            .unwrap();
        assert!(parsed.tree.is_some());
        assert!(!parsed.alternate_variants.is_empty());
        parsed.alternate_variants[0]
            .violations
            .push(sqruff_lib_core::errors::SQLBaseError {
                description: "Alternate branch failed to parse".to_string(),
                rule: Some(sqruff_lib_core::errors::ErrorStructRule {
                    name: "parsing",
                    code: "PRS",
                }),
                ..Default::default()
            });
        let linted = linter.lint_parsed(&tables, parsed, true).unwrap();
        assert!(!linted.violations().iter().any(|v| v.rule_code() == "PRS"));
        assert!(!linted.has_parse_or_templating_errors());
    }

    #[test]
    fn test_jinja_trim_adjacent_whitespace_does_not_create_spurious_variants() {
        let source = std::fs::read_to_string(
            "test/fixtures/templater/jinja_lint_unreached_code/trim_adjacent_whitespace_loop.sql",
        )
        .unwrap();
        let config =
            FluffConfig::from_source("[sqruff]\ndialect = ansi\ntemplater = jinja\n", None);
        let variants = JinjaTemplater
            .process_with_variants(&[(&source, "trim.sql")], &config, &None)
            .remove(0)
            .unwrap();
        assert_eq!(variants.len(), 1);
    }

    #[test]
    fn test_jinja_lints_nested_render_variants() {
        let source =
            std::fs::read_to_string("test/fixtures/linter/jinja_variants/branching_cp01.sql")
                .unwrap();
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = ansi
templater = jinja
rules = CP01
ignore_templated_areas = False

[sqruff:rules:capitalisation.keywords]
capitalisation_policy = upper
"#,
            None,
        );

        let linter = Linter::new(config, None, None, false).unwrap();
        let linted = linter
            .lint_string(&source, Some("branching_cp01.sql".to_string()), false)
            .unwrap();
        let positions = linted
            .violations()
            .iter()
            .filter(|violation| violation.rule_code() == "CP01")
            .map(|violation| (violation.line_no, violation.line_pos))
            .collect::<Vec<_>>();

        assert_eq!(
            positions,
            vec![
                (3, 1),
                (5, 11),
                (7, 11),
                (9, 1),
                (11, 1),
                (11, 15),
                (11, 25),
                (13, 1),
                (13, 15),
                (13, 25),
                (15, 1),
                (15, 15),
                (15, 25),
            ]
        );
    }

    #[test]
    fn test_jinja_nested_render_variants_autofix() {
        let config_source = std::fs::read_to_string(
            "test/fixtures/linter/autofix/ansi/030_jinja_branching_cp01/.sqlfluff",
        )
        .unwrap();
        let config = FluffConfig::from_source(&config_source, None);
        let before = std::fs::read_to_string(
            "test/fixtures/linter/autofix/ansi/030_jinja_branching_cp01/before.sql",
        )
        .unwrap();
        let expected = std::fs::read_to_string(
            "test/fixtures/linter/autofix/ansi/030_jinja_branching_cp01/after.sql",
        )
        .unwrap();

        let linter = Linter::new(config, None, None, false).unwrap();
        let linted = linter
            .lint_string(&before, Some("before.sql".to_string()), true)
            .unwrap();

        assert_eq!(linted.fix_string(), expected);
    }

    #[test]
    #[ignore = "LT02 does not yet indent the first line after a Jinja branch tag"]
    fn test_jinja_fixes_non_conflicting_indentation_in_both_branches() {
        let source =
            "{% if False %}\nSELECT 1\n{% else %}\nSELECT c\nFROM t\nWHERE c < 0\n{% endif %}\n";
        let expected = "{% if False %}\n    SELECT 1\n{% else %}\n    SELECT c\n    FROM t\n    WHERE c < 0\n{% endif %}\n";
        let config = FluffConfig::from_source(
            "[sqruff]\ndialect = ansi\ntemplater = jinja\nrules = LT02\nrender_variant_limit = 5\n",
            None,
        );
        let fixed = Linter::new(config, None, None, false)
            .unwrap()
            .lint_string(source, Some("branches.sql".to_string()), true)
            .unwrap()
            .fix_string();
        assert_eq!(fixed, expected);
    }

    #[test]
    fn test_jinja_placeholder_adjacent_to_quotes_autofix() {
        let source = "SELECT\n  '{{ foo.bar }}'\nFROM baz\n";
        let expected = "SELECT\n    '{{ foo.bar }}'\nFROM baz\n";

        for value in ["", "bar"] {
            let config = FluffConfig::from_source(
                &format!(
                    r#"
[sqruff]
dialect = ansi
templater = jinja
rules = LT02

[sqruff:templater:jinja:context]
foo.bar = {value}
"#
                ),
                None,
            );
            let linter = Linter::new(config, None, None, false).unwrap();
            let linted = linter
                .lint_string(source, Some("test.sql".to_string()), true)
                .unwrap();

            assert_eq!(linted.fix_string(), expected, "context value: {value:?}");
        }
    }

    #[test]
    fn test_jinja_dbt_var_subscript_allows_layout_fix() {
        let sql = "select {{ var('123')['123'] }} ,1/2 as d from d\n";
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = snowflake
rules = LT01
templater = jinja

[sqruff:templater:jinja]
apply_dbt_builtins = True
"#,
            None,
        );
        let linter = Linter::new(config, None, None, false).unwrap();

        let linted = linter
            .lint_string(sql, Some("test.sql".to_string()), true)
            .unwrap();

        assert!(
            !linted
                .violations()
                .iter()
                .any(|violation| violation.rule_code() == "PRS")
        );
        assert_eq!(
            linted.fix_string(),
            "select {{ var('123')['123'] }}, 1 / 2 as d from d\n"
        );
    }

    #[test]
    fn test_jinja_dbt_config_allows_start_of_file_fix() {
        let sql =
            "\n{{\n    config(\n        materialized = \"ephemeral\",\n    )\n}}\n\nSELECT 1\n";
        let config = FluffConfig::from_source(
            "[sqruff]\ndialect = databricks\nrules = LT13\ntemplater = jinja\n\
             [sqruff:templater:jinja]\napply_dbt_builtins = True\n",
            None,
        );
        let linter = Linter::new(config, None, None, false).unwrap();

        let linted = linter
            .lint_string(sql, Some("test.sql".to_string()), true)
            .unwrap();

        assert_eq!(linted.fix_string(), &sql[1..]);
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

    #[test]
    fn test_jinja_templater_dotted_context_config() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
templater = jinja

[sqruff:templater:jinja:context]
namespace.projectname = myproject
namespace.env = prod
"#,
            None,
        );

        let results = JinjaTemplater.process(
            &[(
                "SELECT * FROM `{{ namespace.projectname }}.{{ namespace.env }}_test.table`",
                "test.sql",
            )],
            &config,
            &None,
        );
        let processed = results.into_iter().next().unwrap().unwrap();

        assert_eq!(
            processed.templated(),
            "SELECT * FROM `myproject.prod_test.table`"
        );
    }
}
