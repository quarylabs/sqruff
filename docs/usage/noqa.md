# Ignore errors (noqa)

The NoQA directive disables specific rules or all rules for a line or a range of lines.
Like flake8's ignore, individual lines can be ignored by adding `-- noqa` to the end of the line.

## Ignore single-line errors

The following example ignores all errors on the line where it is placed:

```sql
-- Ignore all errors
SeLeCt  1 from tBl ;    -- noqa

-- Ignore rule CP02 and rule CP03
SeLeCt  1 from tBl ;    -- noqa: CP02,CP03
```

## Ignore multiple line errors

Similar to pylint's directive, ranges of lines can be ignored by adding `-- noqa:disable=<rule>[,...] | all` to the line.
Specified rules (or all rules if `all` was specified) will be ignored until a corresponding `-- noqa:enable=<rule>[,...] | all`.

For example:

```sql
-- Ignore rule AL02 from this line forward
SELECT col_a a FROM foo -- noqa: disable=AL02

-- Ignore all rules from this line forward
SELECT col_a a FROM foo -- noqa: disable=all

-- Enforce all rules from this line forward
SELECT col_a a FROM foo -- noqa: enable=all
```
