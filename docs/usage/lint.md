# Lint and fix

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
