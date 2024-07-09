# Rules

The following rules are available in this create. This list is generated from the `rules` module in the source code and can be turned on or off and configured in the config file. 

## Rule Index

| Rule Code | Rule Name | Description |
|-----------|-----------|-------------|
| AL01 | [aliasing.table](#aliasingtable) | Implicit/explicit aliasing of table. | 
| AL02 | [aliasing.column](#aliasingcolumn) | Implicit/explicit aliasing of columns. | 
| AL03 | [aliasing.expression](#aliasingexpression) | Column expression without alias. Use explicit `AS` clause. | 
| AL04 | [aliasing.unique.table](#aliasinguniquetable) | Table aliases should be unique within each clause. | 
| AL05 | [aliasing.unused](#aliasingunused) | Tables should not be aliased if that alias is not used. | 
| AL06 | [aliasing.lenght](#aliasinglenght) | Identify aliases in from clause and join conditions | 
| AL07 | [aliasing.forbid](#aliasingforbid) | Avoid table aliases in from clauses and join conditions. | 
| AL08 | [layout.cte_newline](#layoutcte_newline) | Column aliases should be unique within each clause. | 
| AL09 | [aliasing.self_alias.column](#aliasingself_aliascolumn) | Find self-aliased columns and fix them | 
| AM01 | [ambiguous.distinct](#ambiguousdistinct) | Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'. | 
| AM02 | [ambiguous.union](#ambiguousunion) | Look for UNION keyword not immediately followed by DISTINCT or ALL | 
| AM06 | [ambiguous.column_references](#ambiguouscolumn_references) | Inconsistent column references in 'GROUP BY/ORDER BY' clauses. | 
| CP01 | [capitalisation.keywords](#capitalisationkeywords) | Inconsistent capitalisation of keywords. | 
| CP02 | [capitalisation.identifiers](#capitalisationidentifiers) | Inconsistent capitalisation of unquoted identifiers. | 
| CP03 | [capitalisation.functions](#capitalisationfunctions) | Inconsistent capitalisation of function names. | 
| CP04 | [capitalisation.literals](#capitalisationliterals) | Inconsistent capitalisation of boolean/null literal. | 
| CP05 | [capitalisation.types](#capitalisationtypes) | Inconsistent capitalisation of datatypes. | 
| CV02 | [convention.coalesce](#conventioncoalesce) | Use 'COALESCE' instead of 'IFNULL' or 'NVL'. | 
| CV03 | [convention.select_trailing_comma](#conventionselect_trailing_comma) | Trailing commas within select clause | 
| CV04 | [convention.count_rows](#conventioncount_rows) | Use consistent syntax to express "count number of rows". | 
| CV08 | [convention.left_join](#conventionleft_join) | Use LEFT JOIN instead of RIGHT JOIN. | 
| LT01 | [layout.spacing](#layoutspacing) | Inappropriate Spacing. | 
| LT02 | [layout.indent](#layoutindent) | Incorrect Indentation. | 
| LT03 | [layout.operators](#layoutoperators) | Operators should follow a standard for being before/after newlines. | 
| LT04 | [layout.commas](#layoutcommas) | Leading/Trailing comma enforcement. | 
| LT05 | [layout.long_lines](#layoutlong_lines) | Line is too long. | 
| LT06 | [layout.functions](#layoutfunctions) | Function name not immediately followed by parenthesis. | 
| LT07 | [layout.cte_bracket](#layoutcte_bracket) | 'WITH' clause closing bracket should be on a new line. | 
| LT08 | [layout.cte_newline](#layoutcte_newline) | Blank line expected but not found after CTE closing bracket. | 
| LT09 | [layout.select_targets](#layoutselect_targets) | Select targets should be on a new line unless there is only one select target. | 
| LT10 | [layout.select_modifiers](#layoutselect_modifiers) | 'SELECT' modifiers (e.g. 'DISTINCT') must be on the same line as 'SELECT'. | 
| LT11 | [layout.set_operators](#layoutset_operators) | Set operators should be surrounded by newlines. | 
| LT12 | [layout.end_of_file](#layoutend_of_file) | Files must end with a single trailing newline. | 
| LT13 | [layout.start_of_file](#layoutstart_of_file) | Files must not begin with newlines or whitespace. | 
| RF01 | [references.from](#referencesfrom) | References cannot reference objects not present in 'FROM' clause. | 
| RF03 | [references.consistent](#referencesconsistent) | References should be consistent in statements with a single table. | 
| RF04 | [references.keywords](#referenceskeywords) | Keywords should not be used as identifiers. | 
| RF05 | [references.special_chars](#referencesspecial_chars) | Do not use special characters in identifiers. | 
| ST01 | [structure.else_null](#structureelse_null) | Do not specify 'else null' in a case when statement (redundant). | 
| ST02 | [structure.simple_case](#structuresimple_case) | Unnecessary 'CASE' statement. | 
| ST03 | [structure.unused_cte](#structureunused_cte) | Query defines a CTE (common-table expression) but does not use it. | 
| ST08 | [structure.distinct](#structuredistinct) | Looking for DISTINCT before a bracket | 

## Rule Details

### aliasing.table

Implicit/explicit aliasing of table.

**Code:** AL01

**Groups:** `all`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

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

**Groups:** `all`, `core`, `aliasing`

**Fixable:** No

**Anti-pattern**

Aliasing the column to itself.

```sql
SELECT
    col AS col
FROM table;
```

**Best practice**

Not to use alias to rename the column to its original name. Self-aliasing leads to redundant code without changing any functionality.

```sql
SELECT
    col
FROM table;
```


### ambiguous.distinct

Ambiguous use of 'DISTINCT' in a 'SELECT' statement with 'GROUP BY'.

**Code:** AM01

**Groups:** `all`, `core`, `ambiguous`

**Fixable:** No

**Anti-pattern**

`DISTINCT` and `GROUP BY are conflicting.

```sql
SELECT DISTINCT
    a
FROM foo
GROUP BY a
```

**Best practice**

Remove `DISTINCT` or `GROUP BY`. In our case, removing `GROUP BY` is better.


```sql
SELECT DISTINCT
    a
FROM foo
```


### ambiguous.union

Look for UNION keyword not immediately followed by DISTINCT or ALL

**Code:** AM02

**Groups:** `all`, `core`, `ambiguous`

**Fixable:** Yes

**Anti-pattern**

In this example, `UNION DISTINCT` should be preferred over `UNION`, because explicit is better than implicit.


```sql
SELECT a, b FROM table_1
UNION
SELECT a, b FROM table_2
```

**Best practice**

Specify `DISTINCT` or `ALL` after `UNION` (note that `DISTINCT` is the default behavior).

```sql
SELECT a, b FROM table_1
UNION DISTINCT
SELECT a, b FROM table_2
```


### ambiguous.column_references

Inconsistent column references in 'GROUP BY/ORDER BY' clauses.

**Code:** AM06

**Groups:** `all`, `core`, `ambiguous`

**Fixable:** No

**Anti-pattern**

In this example, the ORRDER BY clause mixes explicit and implicit order by column references.

```sql
SELECT
    a, b
FROM foo
ORDER BY a, b DESC
```

**Best practice**

If any columns in the ORDER BY clause specify ASC or DESC, they should all do so.

```sql
SELECT
    a, b
FROM foo
ORDER BY a ASC, b DESC
```


### capitalisation.keywords

Inconsistent capitalisation of keywords.

**Code:** CP01

**Groups:** `all`, `core`, `capitalisation`

**Fixable:** Yes

**Anti-pattern**

In this example, select is in lower-case whereas `FROM` is in upper-case.

```sql
select
    a
FROM foo
```

**Best practice**

Make all keywords either in upper-case or in lower-case.

```sql
SELECT
    a
FROM foo

-- Also good

select
    a
from foo
```


### capitalisation.identifiers

Inconsistent capitalisation of unquoted identifiers.

**Code:** CP02

**Groups:** `all`, `core`, `capitalisation`

**Fixable:** Yes

**Anti-pattern**

In this example, unquoted identifier `a` is in lower-case but `B` is in upper-case.

```sql
select
    a,
    B
from foo
```

**Best practice**

Ensure all unquoted identifiers are either in upper-case or in lower-case.

```sql
select
    a,
    b
from foo

-- Also good

select
    A,
    B
from foo
```


### capitalisation.functions

Inconsistent capitalisation of function names.

**Code:** CP03

**Groups:** `all`, `core`, `capitalisation`

**Fixable:** Yes

**Anti-pattern**

In this example, the two `SUM` functions don’t have the same capitalisation.

```sql
SELECT
    sum(a) AS aa,
    SUM(b) AS bb
FROM foo
```

**Best practice**

Make the case consistent.


```sql
SELECT
    sum(a) AS aa,
    sum(b) AS bb
FROM foo
```


### capitalisation.literals

Inconsistent capitalisation of boolean/null literal.

**Code:** CP04

**Groups:** `all`, `core`, `capitalisation`

**Fixable:** Yes

**Anti-pattern**

In this example, `null` and `false` are in lower-case whereas `TRUE` is in upper-case.

```sql
select
    a,
    null,
    TRUE,
    false
from foo
```

**Best practice**

Ensure all literal `null`/`true`/`false` literals are consistently upper or lower case

```sql
select
    a,
    NULL,
    TRUE,
    FALSE
from foo

-- Also good

select
    a,
    null,
    true,
    false
from foo
```


### capitalisation.types

Inconsistent capitalisation of datatypes.

**Code:** CP05

**Groups:** `all`, `core`, `capitalisation`

**Fixable:** Yes

**Anti-pattern**

In this example, `int` and `unsigned` are in lower-case whereas `VARCHAR` is in upper-case.

```sql
CREATE TABLE t (
    a int unsigned,
    b VARCHAR(15)
);
```

**Best practice**

Ensure all datatypes are consistently upper or lower case

```sql
CREATE TABLE t (
    a INT UNSIGNED,
    b VARCHAR(15)
);
```


### convention.coalesce

Use 'COALESCE' instead of 'IFNULL' or 'NVL'.

**Code:** CV02

**Groups:** `all`, `convention`

**Fixable:** No

**Anti-pattern**

`IFNULL` or `NVL` are used to fill `NULL` values.

```sql
SELECT ifnull(foo, 0) AS bar,
FROM baz;

SELECT nvl(foo, 0) AS bar,
FROM baz;
```

**Best practice**

Use COALESCE instead. COALESCE is universally supported, whereas Redshift doesn’t support IFNULL and BigQuery doesn’t support NVL. Additionally, COALESCE is more flexible and accepts an arbitrary number of arguments.

```sql
SELECT coalesce(foo, 0) AS bar,
FROM baz;
```


### convention.select_trailing_comma

Trailing commas within select clause

**Code:** CV03

**Groups:** `all`, `core`, `convention`

**Fixable:** No

**Anti-pattern**

In this example, the last selected column has a trailing comma.

```sql
SELECT
    a,
    b,
FROM foo
```

**Best practice**

Remove the trailing comma.

```sql
SELECT
    a,
    b
FROM foo
```


### convention.count_rows

Use consistent syntax to express "count number of rows".

**Code:** CV04

**Groups:** `all`, `core`, `convention`

**Fixable:** No

**Anti-pattern**

In this example, `count(1)` is used to count the number of rows in a table.

```sql
select
    count(1)
from table_a
```

**Best practice**

Use count(*) unless specified otherwise by config prefer_count_1, or prefer_count_0 as preferred.

```sql
select
    count(*)
from table_a
```


### convention.left_join

Use LEFT JOIN instead of RIGHT JOIN.

**Code:** CV08

**Groups:** `all`, `convention`

**Fixable:** No

**Anti-pattern**

`RIGHT JOIN` is used.

```sql
SELECT
    foo.col1,
    bar.col2
FROM foo
RIGHT JOIN bar
    ON foo.bar_id = bar.id;
```

**Best practice**

Refactor and use ``LEFT JOIN`` instead.

```sql
SELECT
    foo.col1,
    bar.col2
FROM bar
LEFT JOIN foo
   ON foo.bar_id = bar.id;
```


### layout.spacing

Inappropriate Spacing.

**Code:** LT01

**Groups:** `all`, `core`, `layout`

**Fixable:** No

**Anti-pattern**

In this example, spacing is all over the place and is represented by `•`.

```sql
SELECT
    a,        b(c) as d••
FROM foo••••
JOIN bar USING(a)
```

**Best practice**

- Unless an indent or preceding a comment, whitespace should be a single space.
- There should also be no trailing whitespace at the ends of lines.
- There should be a space after USING so that it’s not confused for a function.

```sql
SELECT
    a, b(c) as d
FROM foo
JOIN bar USING (a)
```


### layout.indent

Incorrect Indentation.

**Code:** LT02

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

The ``•`` character represents a space and the ``→`` character represents a tab.
In this example, the third line contains five spaces instead of four and
the second line contains two spaces and one tab.

```sql
SELECT
••→a,
•••••b
FROM foo
```

**Best practice**

Change the indentation to use a multiple of four spaces. This example also assumes that the indent_unit config value is set to space. If it had instead been set to tab, then the indents would be tabs instead.

```sql
SELECT
••••a,
••••b
FROM foo
```


### layout.operators

Operators should follow a standard for being before/after newlines.

**Code:** LT03

**Groups:** `all`, `layout`

**Fixable:** Yes

**Anti-pattern**

In this example, if line_position = leading (or unspecified, as is the default), then the operator + should not be at the end of the second line.

```sql
SELECT
    a +
    b
FROM foo
```

**Best practice**

If line_position = leading (or unspecified, as this is the default), place the operator after the newline.

```sql
SELECT
    a
    + b
FROM foo
```

If line_position = trailing, place the operator before the newline.

```sql
SELECT
    a +
    b
FROM foo
```


### layout.commas

Leading/Trailing comma enforcement.

**Code:** LT04

**Groups:** `all`, `layout`

**Fixable:** Yes

**Anti-pattern**

There is a mixture of leading and trailing commas.

```sql
SELECT
    a
    , b,
    c
FROM foo
```

**Best practice**

By default, sqruff prefers trailing commas. However it is configurable for leading commas. The chosen style must be used consistently throughout your SQL.

```sql
SELECT
    a,
    b,
    c
FROM foo

-- Alternatively, set the configuration file to 'leading'
-- and then the following would be acceptable:

SELECT
    a
    , b
    , c
FROM foo
```


### layout.long_lines

Line is too long.

**Code:** LT05

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

In this example, the line is too long.

```sql
SELECT
    my_function(col1 + col2, arg2, arg3) over (partition by col3, col4 order by col5 rows between unbounded preceding and current row) as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4) as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4) = col9 + col10 as another_relatively_long_alias
FROM my_table
```

**Best practice**

Wraps the line to be within the maximum line length.

```sql
SELECT
    my_function(col1 + col2, arg2, arg3)
        over (
            partition by col3, col4
            order by col5 rows between unbounded preceding and current row
        )
        as my_relatively_long_alias,
    my_other_function(col6, col7 + col8, arg4)
        as my_other_relatively_long_alias,
    my_expression_function(col6, col7 + col8, arg4)
    = col9 + col10 as another_relatively_long_alias
FROM my_table

### layout.functions

Function name not immediately followed by parenthesis.

**Code:** LT06

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

In this example, there is a space between the function and the parenthesis.

```sql
SELECT
    sum (a)
FROM foo
```

**Best practice**

Remove the space between the function and the parenthesis.

```sql
SELECT
    sum(a)
FROM foo
```


### layout.cte_bracket

'WITH' clause closing bracket should be on a new line.

**Code:** LT07

**Groups:** `all`, `core`, `layout`

**Fixable:** No

**Anti-pattern**

In this example, the closing bracket is on the same line as CTE.

```sql
 WITH zoo AS (
     SELECT a FROM foo)

 SELECT * FROM zoo
```

**Best practice**

Move the closing bracket on a new line.

```sql
WITH zoo AS (
    SELECT a FROM foo
)

SELECT * FROM zoo
```


### layout.cte_newline

Blank line expected but not found after CTE closing bracket.

**Code:** LT08

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

There is no blank line after the CTE closing bracket. In queries with many CTEs, this hinders readability.

```sql
WITH plop AS (
    SELECT * FROM foo
)
SELECT a FROM plop
```

**Best practice**

Add a blank line.

```sql
WITH plop AS (
    SELECT * FROM foo
)

SELECT a FROM plop
```


### layout.select_targets

Select targets should be on a new line unless there is only one select target.

**Code:** LT09

**Groups:** `all`, `layout`

**Fixable:** Yes

**Anti-pattern**

Multiple select targets on the same line.

```sql
select a, b
from foo;

-- Single select target on its own line.

SELECT
    a
FROM foo;
```

**Best practice**

Multiple select targets each on their own line.

```sql
select
    a,
    b
from foo;

-- Single select target on the same line as the ``SELECT``
-- keyword.

SELECT a
FROM foo;

-- When select targets span multiple lines, however they
-- can still be on a new line.

SELECT
    SUM(
        1 + SUM(
            2 + 3
        )
    ) AS col
FROM test_table;
```


### layout.select_modifiers

'SELECT' modifiers (e.g. 'DISTINCT') must be on the same line as 'SELECT'.

**Code:** LT10

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

In this example, the `DISTINCT` modifier is on the next line after the `SELECT` keyword.

```sql
select
    distinct a,
    b
from x
```

**Best practice**

Move the `DISTINCT` modifier to the same line as the `SELECT` keyword.

```sql
select distinct
    a,
    b
from x
```


### layout.set_operators

Set operators should be surrounded by newlines.

**Code:** LT11

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

In this example, `UNION ALL` is not on a line itself.

```sql
SELECT 'a' AS col UNION ALL
SELECT 'b' AS col
```

**Best practice**

Place `UNION ALL` on its own line.

```sql
SELECT 'a' AS col
UNION ALL
SELECT 'b' AS col
```


### layout.end_of_file

Files must end with a single trailing newline.

**Code:** LT12

**Groups:** `all`, `core`, `layout`

**Fixable:** Yes

**Anti-pattern**

The content in file does not end with a single trailing newline. The $ represents end of file.

```sql
 SELECT
     a
 FROM foo$

 -- Ending on an indented line means there is no newline
 -- at the end of the file, the • represents space.

 SELECT
 ••••a
 FROM
 ••••foo
 ••••$

 -- Ending on a semi-colon means the last line is not a
 -- newline.

 SELECT
     a
 FROM foo
 ;$

 -- Ending with multiple newlines.

 SELECT
     a
 FROM foo

 $
```

**Best practice**

Add trailing newline to the end. The $ character represents end of file.

```sql
 SELECT
     a
 FROM foo
 $

 -- Ensuring the last line is not indented so is just a
 -- newline.

 SELECT
 ••••a
 FROM
 ••••foo
 $

 -- Even when ending on a semi-colon, ensure there is a
 -- newline after.

 SELECT
     a
 FROM foo
 ;
 $
```


### layout.start_of_file

Files must not begin with newlines or whitespace.

**Code:** LT13

**Groups:** `all`, `layout`

**Fixable:** Yes

**Anti-pattern**

The file begins with newlines or whitespace. The ^ represents the beginning of the file.

```sql
 ^

 SELECT
     a
 FROM foo

 -- Beginning on an indented line is also forbidden,
 -- (the • represents space).

 ••••SELECT
 ••••a
 FROM
 ••••foo
```

**Best practice**

Start file on either code or comment. (The ^ represents the beginning of the file.)

```sql
 ^SELECT
     a
 FROM foo

 -- Including an initial block comment.

 ^/*
 This is a description of my SQL code.
 */
 SELECT
     a
 FROM
     foo

 -- Including an initial inline comment.

 ^--This is a description of my SQL code.
 SELECT
     a
 FROM
     foo
```


### references.from

References cannot reference objects not present in 'FROM' clause.

**Code:** RF01

**Groups:** `all`, `core`, `references`

**Fixable:** No

**Anti-pattern**

In this example, the reference `vee` has not been declared.

```sql
SELECT
    vee.a
FROM foo
```

**Best practice**

Remove the reference.

```sql
SELECT
    a
FROM foo
```


### references.consistent

References should be consistent in statements with a single table.

**Code:** RF03

**Groups:** `all`, `references`

**Fixable:** Yes

**Anti-pattern**

In this example, only the field b is referenced.

```sql
SELECT
    a,
    foo.b
FROM foo
```

**Best practice**

Add or remove references to all fields.

```sql
SELECT
    a,
    b
FROM foo

-- Also good

SELECT
    foo.a,
    foo.b
FROM foo
```


### references.keywords

Keywords should not be used as identifiers.

**Code:** RF04

**Groups:** `all`, `references`

**Fixable:** No

**Anti-pattern**

In this example, `SUM` (a built-in function) is used as an alias.

```sql
SELECT
    sum.a
FROM foo AS sum
```

**Best practice**

Avoid using keywords as the name of an alias.

```sql
SELECT
    vee.a
FROM foo AS vee
```


### references.special_chars

Do not use special characters in identifiers.

**Code:** RF05

**Groups:** `all`, `references`

**Fixable:** No

**Anti-pattern**

Using special characters within identifiers when creating or aliasing objects.

```sql
CREATE TABLE DBO.ColumnNames
(
    [Internal Space] INT,
    [Greater>Than] INT,
    [Less<Than] INT,
    Number# INT
)
```

**Best practice**

Identifiers should include only alphanumerics and underscores.

```sql
CREATE TABLE DBO.ColumnNames
(
    [Internal_Space] INT,
    [GreaterThan] INT,
    [LessThan] INT,
    NumberVal INT
)
```


### structure.else_null

Do not specify 'else null' in a case when statement (redundant).

**Code:** ST01

**Groups:** `all`, `structure`

**Fixable:** No

**Anti-pattern**

In this example, the reference `vee` has not been declared.

```sql
SELECT
    vee.a
FROM foo
```

**Best practice**

Remove the reference.

```sql
SELECT
    a
FROM foo
```


### structure.simple_case

Unnecessary 'CASE' statement.

**Code:** ST02

**Groups:** `all`, `structure`

**Fixable:** No

**Anti-pattern**

CASE statement returns booleans.

```sql
select
    case
        when fab > 0 then true
        else false
    end as is_fab
from fancy_table

-- This rule can also simplify CASE statements
-- that aim to fill NULL values.

select
    case
        when fab is null then 0
        else fab
    end as fab_clean
from fancy_table

-- This also covers where the case statement
-- replaces NULL values with NULL values.

select
    case
        when fab is null then null
        else fab
    end as fab_clean
from fancy_table
```

**Best practice**

Reduce to WHEN condition within COALESCE function.

```sql
select
    coalesce(fab > 0, false) as is_fab
from fancy_table

-- To fill NULL values.

select
    coalesce(fab, 0) as fab_clean
from fancy_table

-- NULL filling NULL.

select fab as fab_clean
from fancy_table
```


### structure.unused_cte

Query defines a CTE (common-table expression) but does not use it.

**Code:** ST03

**Groups:** `all`, `core`, `structure`

**Fixable:** No

**Anti-pattern**

Defining a CTE that is not used by the query is harmless, but it means the code is unnecessary and could be removed.

```sql
WITH cte1 AS (
  SELECT a
  FROM t
),
cte2 AS (
  SELECT b
  FROM u
)

SELECT *
FROM cte1
```

**Best practice**

Remove unused CTEs.

```sql
WITH cte1 AS (
  SELECT a
  FROM t
)

SELECT *
FROM cte1
```


### structure.distinct

Looking for DISTINCT before a bracket

**Code:** ST08

**Groups:** `all`, `core`, `structure`

**Fixable:** No

**Anti-pattern**

In this example, parentheses are not needed and confuse DISTINCT with a function. The parentheses can also be misleading about which columns are affected by the DISTINCT (all the columns!).

```sql
SELECT DISTINCT(a), b FROM foo
```

**Best practice**

Remove parentheses to be clear that the DISTINCT applies to both columns.

```sql
SELECT DISTINCT a, b FROM foo
```

