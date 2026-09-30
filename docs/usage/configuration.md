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
sql_file_exts = .sql,.sql.j2,.dml,.ddl,.pkb

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
sql_file_exts = ".sql,.sql.j2,.dml,.ddl,.pkb"

[tool.sqlfluff.indentation]
indent_unit = "space"
tab_space_size = 4
indented_joins = true
```

See [sample configurations](../reference/sample-configurations.md) for more examples.

The `warnings` setting makes selected violations visible without causing lint
to fail. It accepts either rule codes or rule names, for example
`warnings = LT01,layout.end_of_file`.

## Parser resource limits

Untrusted SQL can exhaust parser resources with deeply nested or unusually
large query structures. Keep `max_parse_depth` and `max_parse_nodes` enabled
when linting SQL from untrusted sources. Their defaults are `255` and `100000`,
respectively, and they limit parser nesting and parse-tree size. Projects with
legitimately complex queries can raise either limit; setting a limit to `0`
disables it.

## Jinja branch variants

With `templater = jinja`, sqruff renders up to five branches of a template by
default. Lint findings from those branches are combined, and compatible fixes
from multiple branches can be applied to the source file. If a branch cannot
be parsed, its parse error does not invalidate a different branch that can be
parsed.

Set `render_variant_limit` under `[sqruff]` to change the maximum number of
renderings. A value of `1` checks only the first rendering; higher values can
increase templating and linting time.

```ini
[sqruff]
templater = jinja
render_variant_limit = 5
```

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

## Attaching operators to adjacent lines

The `leading:attached` and `trailing:attached` modifiers allow an operator in
the middle of a line, but do not allow it on a line by itself. With
`trailing:attached`, an operator before a line break stays with the preceding
token; with `leading:attached`, it stays with the following token. For example:

```ini
[sqruff:layout:type:assignment_operator]
line_position = trailing:attached
```

For `my_var := some_value`, this permits either a single line or `my_var :=`
followed by `some_value` on the next line, but not a standalone `:=`, nor one
at the start of the next line. Use `leading:attached` to require the opposite
side of a line break instead.

## Segment and keyword line positions

The `line_position` setting applies to a configured segment type as a whole,
such as where a `where_clause` should break relative to the surrounding SQL.
The `keyword_line_position` setting applies to the clause's leading keyword
tokens, such as `WHERE` or `ORDER BY`, inside that segment.

For clause-like types, the settings are often used together. For example:

```ini
[sqruff:layout:type:where_clause]
line_position = alone
keyword_line_position = leading
```

This treats the `WHERE` clause as its own line-oriented block and requires the
`WHERE` keyword to start a line. In the following SQL, `line_position` controls
where the whole clause sits relative to `FROM`, while `keyword_line_position`
controls where `WHERE` sits relative to its expression:

```sql
SELECT
    a
FROM t
WHERE b = 1
    AND c = 2
```

Setting `keyword_line_position = alone` instead puts the expression on the
following line:

```sql
SELECT
    a
FROM t
WHERE
    b = 1
    AND c = 2
```

Keyword positioning also supports trailing placement. For example:

```ini
[sqruff:layout:type:join_on_condition]
keyword_line_position = trailing
```

This places the `ON` keyword at the end of the line, with the condition
expression on the following line.

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
