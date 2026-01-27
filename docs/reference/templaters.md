# Templaters

Templaters allow you to format/lint non-standard SQL code with sqruff that is transformed in some application setting.
An example of this may be SQL code that is templated with dynamic parameters, for example the below `:id` is replaced at
runtime.

```sql
SELECT id, name
FROM users
WHERE id = :id
```

The templater is set in the config file as follows:

```ini
[sqruff]
templater = raw
```

## Templaters Index

Sqruff comes with the following templaters out of the box:

- [raw](#raw)
- [placeholder](#placeholder)
- [python](#python)
- [jinja](#jinja)
- [dbt](#dbt)

## Details

### raw

The raw templater simply returns the input string as the output string. It passes through the input string unchanged and is useful if you need no templating. It is the default templater.

### placeholder

Libraries such as SQLAlchemy or Psycopg use different parameter placeholder styles to mark where a parameter has to be inserted in the query.

For example a query in SQLAlchemy can look like this:

```sql
SELECT * FROM table WHERE id = :myid
```

At runtime :myid will be replace by a value provided by the application and escaped as needed, but this is not standard SQL and cannot be parsed as is.

In order to parse these queries is then necessary to replace these placeholders with sample values, and this is done with the placeholder templater.

Placeholder templating can be enabled in the config using:

```ini
[sqruff]
templater = placeholder
```

A few common styles are supported:

```sql
 -- colon
 WHERE bla = :my_name

 -- colon_nospaces
 -- (use with caution as more prone to false positives)
 WHERE bla = table:my_name

 -- colon_optional_quotes
 SELECT :"column" FROM :table WHERE bla = :'my_name'

 -- numeric_colon
 WHERE bla = :2

 -- pyformat
 WHERE bla = %(my_name)s

 -- dollar
 WHERE bla = $my_name or WHERE bla = ${my_name}

 -- question_mark
 WHERE bla = ?

 -- numeric_dollar
 WHERE bla = $3 or WHERE bla = ${3}

 -- percent
 WHERE bla = %s

 -- ampersand
 WHERE bla = &s or WHERE bla = &{s} or USE DATABASE MARK_{ENV}

 -- apache_camel
 WHERE bla = :#${qwe}

 -- at
 WHERE bla = @my_name
```

The can be configured by setting `param_style` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_style = colon
my_name = 'john'
```

then you can set sample values for each parameter, like my_name above. Notice that the value needs to be escaped as it will be replaced as a string during parsing. When the sample values aren't provided, the templater will use parameter names themselves by default.

When parameters are positional, like question_mark, then their name is simply the order in which they appear, starting with 1.

```ini
[sqruff:templater:placeholder]
param_style = question_mark
1 = 'john'
```

In case you nbeed a parameter style different from the ones provided, you can set `param_regex` in the config file. For example:

```ini
[sqruff:templater:placeholder]
param_regex = __(?P<param_name>[\w_]+)__
my_name = 'john'
```

N.B. quotes around param_regex in the config are interpreted literally by the templater. e.g. param_regex='__(?P<param_name>[w_]+)__' matches '__some_param__' not __some_param__

the named parameter param_name will be used as the key to replace, if missing, the parameter is assumed to be positional and numbers are used instead.

Also consider making a pull request to the project to have your style added, it may be useful to other people and simplify your configuration.

### python

**Note:** This templater currently does not work by default in the CLI and needs custom set up to work.

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

At the moment, dot notation is not supported in the templater.

### jinja

The jinja templater uses the Jinja2 templating engine to process SQL files with dynamic content. This is useful for SQL that uses variables, loops, conditionals, and macros.

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

When `apply_dbt_builtins` is enabled (the default), common dbt functions like `ref()`, `source()`, and `config()` are available as dummy implementations. This allows linting dbt-style SQL without a full dbt project setup. For full dbt support, use the `dbt` templater instead.

### dbt

The dbt templater processes dbt models by compiling them using the dbt-core library. This provides full dbt functionality including proper resolution of `ref()`, `source()`, macros, and other dbt features.

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

The linter then operates on this compiled SQL.
