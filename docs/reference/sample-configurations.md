# Sample Configurations

The following document outlines sample configurations that may be used to achieve certain formatting/linting outcomes.

## Aligning AS statements

Suppose you want to align as statements in a `select` to return the following outcome.

```sql
--before
select
    aaaa as a,
    bbb as b,
    cc as c
from table;

--after
select
    aaaa as a,
    bbb  as b,
    cc   as c
from table
```

This can be achieved with the following configuration addition:

```
[sqruff:layout:type:alias_expression]
spacing_before = align
align_within = select_clause
align_scope = bracketed
```
