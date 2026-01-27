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
        r#"The dbt templater processes dbt models by compiling them using the dbt-core library. This provides full dbt functionality including proper resolution of `ref()`, `source()`, macros, and other dbt features.

**Note:** This templater requires Python with dbt-core and the sqruff Python package. Install them with:

```bash
pip install sqruff dbt-core
```

You'll also need the appropriate dbt adapter for your database (e.g., `dbt-snowflake`, `dbt-bigquery`, `dbt-postgres`).

Alternatively, build sqruff from source with the `python` feature enabled.

## Activation

Enable the dbt templater in your `.sqruff` config file:

```ini
[sqruff]
templater = dbt
```

## Configuration Options

Configuration options are set in the `[sqruff:templater:dbt]` section:

```ini
[sqruff:templater:dbt]
# Path to your dbt project directory (default: current working directory)
project_dir = ./my_dbt_project

# Path to your dbt profiles directory (default: ~/.dbt)
profiles_dir = ~/.dbt

# Specify a profile name (optional, uses default from dbt_project.yml)
profile = my_profile

# Specify a target name (optional, uses default from profiles.yml)
target = dev
```

## dbt Variables

Pass dbt variables via the context section. These are equivalent to using `--vars` on the command line:

```ini
[sqruff:templater:dbt:context]
my_var = some_value
start_date = 2024-01-01
```

These variables are then accessible in your dbt models via `{{ var('my_var') }}`.

## Requirements

For the dbt templater to work correctly, you need:

1. A valid dbt project with `dbt_project.yml`
2. A `profiles.yml` file with database connection details
3. A compiled dbt manifest (run `dbt compile` or `dbt run` first)

## How It Works

The dbt templater:

1. Loads your dbt project configuration and manifest
2. Identifies the model corresponding to each SQL file
3. Compiles the model using dbt's compiler (resolving refs, sources, macros)
4. Returns the compiled SQL for linting

## Ephemeral Models

The templater automatically handles ephemeral model dependencies by processing them in the correct order. Files are sequenced based on their dependency graph to ensure proper compilation.

## Database Connection

Note that dbt may need to connect to your database during compilation (e.g., for `run_query` macros or adapter-specific operations). Ensure your database credentials are correctly configured in `profiles.yml`.

If you encounter connection errors, try running `dbt debug` to verify your setup.

## Example

With the dbt templater enabled, a model like:

```sql
SELECT *
FROM {{ ref('stg_users') }}
WHERE created_at > '{{ var("start_date") }}'
```

Will be compiled to something like:

```sql
SELECT *
FROM "database"."schema"."stg_users"
WHERE created_at > '2024-01-01'
```

The linter then operates on this compiled SQL."#
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

            let python_fluff_config: PythonFluffConfig = config.clone().into();

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
