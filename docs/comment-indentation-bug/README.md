# Comment-Induced Indentation Bug

A comment placed as the first line inside a parenthesized expression could cause the
next line to receive an extra level of indentation. For example:

```sql
SELECT
    col1,
    (
        -- BIG NOTE 1: Adding this comment breaks the indentation of the output.
        file_type ILIKE '%tif%'
    ) AS col2
```

Running `sqruff fix` on this query previously produced:

```sql
SELECT
    col1,
    (
        -- BIG NOTE 1: Adding this comment breaks the indentation of the output.
            file_type ILIKE '%tif%'
        ) AS col2
```

The comment-only line after the opening parenthesis was treated as consuming an
additional indent. The following line then inherited this inflated indent balance,
shifting the subsequent lines too far to the right.

## Fix

The indentation mapper now resets comment-only lines to the preceding line's
indentation and clears any untaken indents. As a result, comment lines no longer
alter the indent level of the code that follows them:

```sql
SELECT
    col1,
    (
        -- BIG NOTE 1: Adding this comment breaks the indentation of the output.
        file_type ILIKE '%tif%'
    ) AS col2
```
