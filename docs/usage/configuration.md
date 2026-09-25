# Configuration

Settings for SQL dialect, indentation, capitalization, and other linting and style options are configured in a `.sqruff`, `.sqruff.ini`, `sqruff.toml`, or `pyproject.toml` file.
Sqruff loads configuration files from the user's home directory and each
directory down to the directory where sqruff is run. When linting a file in a
subdirectory, it then loads configuration from each directory down to the
file. Settings found closer to the file override settings from outer
directories.

The following example highlights a few configuration points: setting the dialect to `sqlite`, turning on all rules except AM01 and AM02, and configuring some indentation settings.
For a comprehensive list of configuration options, see the [default configuration file](https://github.com/quarylabs/sqruff/blob/main/crates/lib/src/core/default_config.cfg).
You can also refer to the [rules documentation](../reference/rules.md) for more information on configuring specific rules.

```ini
[sqruff]
dialect = sqlite
exclude_rules = AM01,AM02
rules = all

[sqruff:indentation]
indent_unit = space
tab_space_size = 4
indented_joins = True
```

The same configuration can be written in TOML:

```toml
[tool.sqlfluff.core]
dialect = "sqlite"
exclude_rules = ["AM01", "AM02"]
rules = "all"

[tool.sqlfluff.indentation]
indent_unit = "space"
tab_space_size = 4
indented_joins = true
```

See [sample configurations](../reference/sample-configurations.md) for more examples.

The `warnings` setting makes selected violations visible without causing lint
to fail. It accepts either rule codes or rule names, for example
`warnings = LT01,layout.end_of_file`.

## Implicit indents

Set `implicit_indents` in the `indentation` section to `allow` to accept implicit
indents, or to `require` to collapse them. When using `require`, the
`skip_implicit_indents_in` option excludes specific element types from
collapsing and defaults to `case_expression`.

## Aligning after leading punctuation

The `leading:align-following` line-position modifier aligns the element after a
leading comma or binary operator with the surrounding expressions. For example:

```ini
[sqruff:layout:type:comma]
line_position = leading:align-following
```

With that configuration, this indentation is valid:

```sql
SELECT
   col_a AS a
 , col_b AS b
FROM foo;
```

The same modifier can be configured for `binary_operator`.

## Keyword line position exclusions

LT14 supports `keyword_line_position_exclusions`, a comma-separated list of
ancestor segment types whose keywords should be left in place. For example,
keep `ORDER BY` inline inside window specifications and aggregate functions
while requiring an outer `ORDER BY` to start a line:

```ini
[sqruff:layout:type:orderby_clause]
keyword_line_position = leading
keyword_line_position_exclusions = window_specification, aggregate_order_by
```

With this configuration, the following passes LT14:

```sql
SELECT
    ROW_NUMBER() OVER (PARTITION BY c ORDER BY d) AS e,
    STRING_AGG(a ORDER BY b, c)
FROM f
ORDER BY e
```

Writing `FROM f ORDER BY e` instead would fail LT14, which moves the outer
`ORDER BY` onto a new line. The exclusions apply only inside the specified
ancestor segments. Use `keyword_line_position_exclusions = None` to clear
inherited exclusions and apply keyword positioning inside those segments too.
