# Rules

The following rules are available in this create. This list is generated from the `rules` module in the source code and can be turned on or off and configured in the config file. 

## Rule Index

| Rule Code | Rule Name | 
|-----------|-----------|
| AL01 | [aliasing.table](#aliasing.table) |
| AL02 | [aliasing.column](#aliasing.column) |
| AL03 | [aliasing.expression](#aliasing.expression) |
| AL04 | [aliasing.unique.table](#aliasing.unique.table) |
| AL05 | [aliasing.unused](#aliasing.unused) |
| AL06 | [aliasing.lenght](#aliasing.lenght) |
| AL07 | [aliasing.forbid](#aliasing.forbid) |
| AL08 | [layout.cte_newline](#layout.cte_newline) |
| AL09 | [aliasing.self_alias.column](#aliasing.self_alias.column) |
| AM01 | [ambiguous.distinct](#ambiguous.distinct) |
| AM02 | [ambiguous.union](#ambiguous.union) |
| AM06 | [ambiguous.column_references](#ambiguous.column_references) |
| CP01 | [capitalisation.keywords](#capitalisation.keywords) |
| CP02 | [capitalisation.identifiers](#capitalisation.identifiers) |
| CP03 | [capitalisation.functions](#capitalisation.functions) |
| CP04 | [capitalisation.literals](#capitalisation.literals) |
| CP05 | [capitalisation.types](#capitalisation.types) |
| CV02 | [convention.coalesce](#convention.coalesce) |
| CV03 | [convention.select_trailing_comma](#convention.select_trailing_comma) |
| CV04 | [convention.count_rows](#convention.count_rows) |
| LT01 | [layout.spacing](#layout.spacing) |
| LT02 | [layout.indent](#layout.indent) |
| LT03 | [layout.operators](#layout.operators) |
| LT04 | [layout.commas](#layout.commas) |
| LT05 | [layout.long_lines](#layout.long_lines) |
| LT06 | [layout.functions](#layout.functions) |
| LT07 | [layout.cte_bracket](#layout.cte_bracket) |
| LT08 | [layout.cte_newline](#layout.cte_newline) |
| LT09 | [layout.select_targets](#layout.select_targets) |
| LT10 | [layout.select_modifiers](#layout.select_modifiers) |
| LT11 | [layout.set_operators](#layout.set_operators) |
| LT12 | [layout.end_of_file](#layout.end_of_file) |
| RF01 | [references.from](#references.from) |
| RF03 | [references.consistent](#references.consistent) |
| ST01 | [structure.else_null](#structure.else_null) |
| ST02 | [structure.simple_case](#structure.simple_case) |
| ST03 | [structure.unused_cte](#structure.unused_cte) |
| ST08 | [structure.distinct](#structure.distinct) |

## Rule Details

### aliasing.table

Implicit/explicit aliasing of table.

**Code:** AL01

**Fixable:** Yes


**Anti-pattern**

In this example, the alias `voo` is implicit.

```sql
SELECT
    voo.a
FROM foo voo
```

**Best practice**

Add `AS` to make the alias explicit.

```sql
SELECT
    voo.a
FROM foo AS voo
```


### aliasing.column

Implicit/explicit aliasing of columns.

**Code:** AL02

**Fixable:** No


**Anti-pattern**

In this example, the alias for column `a` is implicit.

```sql
SELECT
  a alias_col
FROM foo
```

**Best practice**

Add the `AS` keyword to make the alias explicit.

```sql
SELECT
    a AS alias_col
FROM foo
```


### aliasing.expression

Column expression without alias. Use explicit `AS` clause.

**Code:** AL03

**Fixable:** No


**Anti-pattern**

In this example, there is no alias for both sums.

```sql
SELECT
    sum(a),
    sum(b)
FROM foo
```

**Best practice**

Add aliases.

```sql
SELECT
    sum(a) AS a_sum,
    sum(b) AS b_sum
FROM foo
```


### aliasing.unique.table

Table aliases should be unique within each clause.

**Code:** AL04

**Fixable:** No


**Anti-pattern**

In this example, the alias t is reused for two different tables:

```sql
SELECT
    t.a,
    t.b
FROM foo AS t, bar AS t

-- This can also happen when using schemas where the
-- implicit alias is the table name:

SELECT
    a,
    b
FROM
    2020.foo,
    2021.foo
```

**Best practice**

Make all tables have a unique alias.

```sql
SELECT
    f.a,
    b.b
FROM foo AS f, bar AS b

-- Also use explicit aliases when referencing two tables
-- with the same name from two different schemas.

SELECT
    f1.a,
    f2.b
FROM
    2020.foo AS f1,
    2021.foo AS f2
```


### aliasing.unused

Tables should not be aliased if that alias is not used.

**Code:** AL05

**Fixable:** Yes


**Anti-pattern**

In this example, alias `zoo` is not used.

```sql
SELECT
    a
FROM foo AS zoo
```

**Best practice**

Use the alias or remove it. An unused alias makes code harder to read without changing any functionality.

```sql
SELECT
    zoo.a
FROM foo AS zoo

-- Alternatively...

SELECT
    a
FROM foo
```


### aliasing.lenght

Identify aliases in from clause and join conditions

**Code:** AL06

**Fixable:** No


**Anti-pattern**

In this example, alias `o` is used for the orders table.

```sql
SELECT
    SUM(o.amount) as order_amount,
FROM orders as o
```

**Best practice**

Avoid aliases. Avoid short aliases when aliases are necessary.

See also: Rule_AL07.

```sql
SELECT
    SUM(orders.amount) as order_amount,
FROM orders

SELECT
    replacement_orders.amount,
    previous_orders.amount
FROM
    orders AS replacement_orders
JOIN
    orders AS previous_orders
    ON replacement_orders.id = previous_orders.replacement_id
```


### aliasing.forbid

Avoid table aliases in from clauses and join conditions.

**Code:** AL07

**Fixable:** Yes


**Anti-pattern**

In this example, alias o is used for the orders table, and c is used for customers table.

```sql
SELECT
    COUNT(o.customer_id) as order_amount,
    c.name
FROM orders as o
JOIN customers as c on o.id = c.user_id
```

**Best practice**

Avoid aliases.

```sql
SELECT
    COUNT(orders.customer_id) as order_amount,
    customers.name
FROM orders
JOIN customers on orders.id = customers.user_id

-- Self-join will not raise issue

SELECT
    table1.a,
    table_alias.b,
FROM
    table1
    LEFT JOIN table1 AS table_alias ON
        table1.foreign_key = table_alias.foreign_key
```


### layout.cte_newline

Column aliases should be unique within each clause.

**Code:** AL08

**Fixable:** No


**Anti-pattern**

In this example, alias o is used for the orders table, and c is used for customers table.

```sql
SELECT
    COUNT(o.customer_id) as order_amount,
    c.name
FROM orders as o
JOIN customers as c on o.id = c.user_id
```

**Best practice**

Avoid aliases.

```sql
SELECT
    COUNT(orders.customer_id) as order_amount,
    customers.name
FROM orders
JOIN customers on orders.id = customers.user_id

-- Self-join will not raise issue

SELECT
    table1.a,
    table_alias.b,
FROM
    table1
    LEFT JOIN table1 AS table_alias ON
        table1.foreign_key = table_alias.foreign_key
```


### aliasing.self_alias.column

Find self-aliased columns and fix them

**Code:** AL09

**Fixable:** No



### ambiguous.distinct

Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'.

**Code:** AM01

**Fixable:** No



### ambiguous.union

Look for UNION keyword not immediately followed by DISTINCT or ALL

**Code:** AM02

**Fixable:** Yes



### ambiguous.column_references

Inconsistent column references in 'GROUP BY/ORDER BY' clauses.

**Code:** AM06

**Fixable:** No



### capitalisation.keywords

Inconsistent capitalisation of keywords.

**Code:** CP01

**Fixable:** Yes



### capitalisation.identifiers

Inconsistent capitalisation of unquoted identifiers.

**Code:** CP02

**Fixable:** Yes



### capitalisation.functions

Inconsistent capitalisation of function names.

**Code:** CP03

**Fixable:** Yes



### capitalisation.literals

Inconsistent capitalisation of boolean/null literal.

**Code:** CP04

**Fixable:** Yes



### capitalisation.types

Inconsistent capitalisation of datatypes.

**Code:** CP05

**Fixable:** Yes



### convention.coalesce

Use 'COALESCE' instead of 'IFNULL' or 'NVL'.

**Code:** CV02

**Fixable:** No



### convention.select_trailing_comma

Trailing commas within select clause

**Code:** CV03

**Fixable:** No



### convention.count_rows

Use consistent syntax to express "count number of rows".

**Code:** CV04

**Fixable:** No



### layout.spacing

Inappropriate Spacing.

**Code:** LT01

**Fixable:** No



### layout.indent

Incorrect Indentation.

**Code:** LT02

**Fixable:** Yes



### layout.operators

Operators should follow a standard for being before/after newlines.

**Code:** LT03

**Fixable:** Yes



### layout.commas

Leading/Trailing comma enforcement.

**Code:** LT04

**Fixable:** Yes



### layout.long_lines

Line is too long.

**Code:** LT05

**Fixable:** Yes



### layout.functions

Function name not immediately followed by parenthesis.

**Code:** LT06

**Fixable:** Yes



### layout.cte_bracket

'WITH' clause closing bracket should be on a new line.

**Code:** LT07

**Fixable:** No



### layout.cte_newline

Blank line expected but not found after CTE closing bracket.

**Code:** LT08

**Fixable:** Yes



### layout.select_targets

Select targets should be on a new line unless there is only one select target.

**Code:** LT09

**Fixable:** Yes



### layout.select_modifiers

'SELECT' modifiers (e.g. 'DISTINCT') must be on the same line as 'SELECT'.

**Code:** LT10

**Fixable:** Yes



### layout.set_operators

Set operators should be surrounded by newlines.

**Code:** LT11

**Fixable:** Yes



### layout.end_of_file

Files must end with a single trailing newline.

**Code:** LT12

**Fixable:** Yes



### references.from

References cannot reference objects not present in 'FROM' clause.

**Code:** RF01

**Fixable:** No



### references.consistent

References should be consistent in statements with a single table.

**Code:** RF03

**Fixable:** Yes



### structure.else_null

Do not specify 'else null' in a case when statement (redundant).

**Code:** ST01

**Fixable:** No



### structure.simple_case

Unnecessary 'CASE' statement.

**Code:** ST02

**Fixable:** No



### structure.unused_cte

Query defines a CTE (common-table expression) but does not use it.

**Code:** ST03

**Fixable:** No



### structure.distinct

Looking for DISTINCT before a bracket

**Code:** ST08

**Fixable:** No


