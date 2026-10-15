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
