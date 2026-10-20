# The CLI

## Lint

Use `sqruff lint` to analyze SQL files or directories.

```bash
sqruff lint <file>
sqruff lint <file1> <file2> <file3>
sqruff lint <directory>
```

## Fix

Use `sqruff fix` to format and fix linting issues in SQL files or directories.

```bash
sqruff fix <file/paths/directory>
```

## Inspect the concrete syntax tree

Use `sqruff parse` to inspect SQL segment types and raw text in the concrete
syntax tree (CST). This command is available in standard CLI builds.

```bash
sqruff parse query.sql --dialect ansi
echo 'SELECT a, b FROM example' | sqruff parse --dialect ansi -
sqruff parse query.sql --dialect ansi --format json --include-meta
```

Omitting the file paths also reads from stdin. The default `pretty` format
prints a labeled tree; `json` and `yaml` provide structured output. Add
`--include-meta` to include meta segments and source positions in JSON or YAML.
Parse errors are reported on stderr with a nonzero exit code, while the
available tree is printed on stdout.

See the collapsed segment types lists in the [dialect reference](../reference/dialects.md)
for automatically generated names used in layout configuration.

## Snowflake SQL procedure bodies

With the `snowflake` dialect, sqruff also lints and formats SQL inside quoted
procedure bodies. This applies to `LANGUAGE SQL` and procedures that omit
`LANGUAGE`, which defaults to SQL. Both `$$...$$` and single-quoted bodies are
supported, using the same rule and indentation configuration as the containing
file. Diagnostics point to the original file, and fixes preserve the body's
delimiters and string values.

Bodies in other languages and SQL strings passed to `EXECUTE IMMEDIATE` are
preserved. Bodies containing template expansions are currently preserved too.
If the Snowflake parser cannot parse a body, sqruff leaves it unchanged and
reports parse errors when parse error reporting is enabled.
