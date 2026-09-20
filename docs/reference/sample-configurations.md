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

When templated expressions participate in an alignment group, sqruff uses their
source positions by default. This keeps the visible template aligned instead of
adding padding based on the rendered value. You can force a coordinate space
for a layout type when necessary:

```ini
[sqruff:layout:type:alias_expression]
spacing_before = align
align_within = select_clause
align_scope = bracketed
alignment_coordinate_space = source
```

Supported values are `source` and `templated`. The same override can be written
as an alignment suffix:

```ini
spacing_before = align:alias_expression:select_clause:bracketed:templated
```
