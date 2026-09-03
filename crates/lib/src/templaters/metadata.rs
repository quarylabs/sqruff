#[derive(Debug, Clone, Copy)]
pub struct TemplaterMetadata {
    pub name: &'static str,
    pub description: &'static str,
}

pub const PYTHON_TEMPLATER: TemplaterMetadata = TemplaterMetadata {
    name: "python",
    description: r"**Note:** This templater currently does not work by default in the CLI and needs custom set up to work.

The Python templater uses native Python f-strings. An example would be as follows:

```sql
SELECT * FROM {blah}
```

With the following config:

```
[sqruff]
templater = python

[sqruff:templater:python:context]
blah = foo
```

Before parsing the sql will be transformed to:

```sql
SELECT * FROM foo
```

At the moment, dot notation is not supported in the templater.",
};

pub const JINJA_TEMPLATER: TemplaterMetadata = TemplaterMetadata {
    name: "jinja",
    description: r#"The jinja templater uses the Jinja2 templating engine to process SQL files with dynamic content. This is useful for SQL that uses variables, loops, conditionals, and macros.

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
```"#,
};

pub const DBT_TEMPLATER: TemplaterMetadata = TemplaterMetadata {
    name: "dbt",
    description: r#"The dbt templater processes dbt models by compiling them using the dbt-core library. This provides full dbt functionality including proper resolution of `ref()`, `source()`, macros, and other dbt features.

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

The linter then operates on this compiled SQL."#,
};

pub const PYTHON_TEMPLATERS: [TemplaterMetadata; 3] =
    [PYTHON_TEMPLATER, JINJA_TEMPLATER, DBT_TEMPLATER];
