# SQL Column Lineage

Sqruff includes a column lineage analysis feature that traces data flow through SQL queries. This helps you understand how columns originate from source tables and how they transform through query execution.

## Overview

Column lineage analysis answers questions like:

- Where does this column come from?
- What source tables contribute to this column?
- How does data flow through CTEs and subqueries?

## Availability

The lineage feature is currently available through:

- **Web Playground**: Try it at [playground.quary.dev](https://playground.quary.dev) by selecting the "Lineage" tool
- **Rust API**: For programmatic use in Rust applications

## Using the Playground

1. Go to [playground.quary.dev](https://playground.quary.dev)
2. Select "Lineage" from the tool dropdown
3. Enter your SQL query
4. View the lineage tree output

## Features

### Column Tracing

Track columns through multiple levels of nesting. The lineage analyzer follows column references from the outermost query down to the source tables.

```sql
SELECT a FROM z
-- Traces 'a' back through any CTEs, subqueries, or source tables
```

### CTE Support

Common Table Expressions are fully supported, including multiple CTE references:

```sql
WITH
  cte1 AS (SELECT id, name FROM users),
  cte2 AS (SELECT id, amount FROM orders)
SELECT c1.name, c2.amount
FROM cte1 c1
JOIN cte2 c2 ON c1.id = c2.id
```

### UNION Queries

Lineage tracks columns through UNION operations, showing all contributing branches:

```sql
SELECT x FROM a
UNION
SELECT x FROM b
UNION
SELECT x FROM c
-- Shows lineage from all three sources
```

### Subqueries

Both derived table subqueries and scalar subqueries are supported:

```sql
-- Derived table
SELECT x FROM (SELECT x FROM source_table) sub

-- Scalar subquery
SELECT (SELECT max(value) FROM metrics) AS max_value FROM dual
```

### JOIN Handling

Columns are correctly traced through JOIN operations:

```sql
SELECT u.name, t.id
FROM users AS u
INNER JOIN tests AS t ON u.id = t.id
```

### SELECT \* Expansion

Star projections are handled and can be traced to their source tables:

```sql
WITH y AS (SELECT * FROM x)
SELECT a FROM y
-- Traces 'a' through the star expansion back to table x
```

## Rust API

For Rust applications, use the `lineage` crate directly:

```rust
use lineage::Lineage;
use sqruff_lib_core::parser::Parser;
use std::collections::HashMap;

// Create a parser for your dialect
let dialect = sqruff_lib_dialects::ansi::dialect();
let parser = Parser::new(&dialect, Default::default());

// Build the lineage for a specific column
let (tables, root_node) = Lineage::new(parser, "column_name", "SELECT column_name FROM my_table")
    .build();

// Access node data
let node_data = &tables.nodes[root_node];
println!("Column: {}", node_data.name);
println!("Source: {}", tables.stringify(node_data.source));

// Traverse downstream nodes (dependencies)
for &child in &node_data.downstream {
    let child_data = &tables.nodes[child];
    println!("  Depends on: {}", child_data.name);
}
```

### Builder Methods

The `Lineage` builder provides several configuration options:

| Method                     | Description                                         |
| -------------------------- | --------------------------------------------------- |
| `new(parser, column, sql)` | Create a new lineage analyzer for a column          |
| `schema(name, columns)`    | Register table schema (column name to type mapping) |
| `source(name, sql)`        | Register a source table's SQL definition            |
| `disable_trim_selects()`   | Keep intermediate SELECT projections in output      |
| `build()`                  | Execute analysis and return `(Tables, Node)`        |

### Registering Source Tables

You can provide SQL definitions for source tables to enable deeper lineage tracing:

```rust
let (tables, node) = Lineage::new(parser, "a", "SELECT a FROM z")
    .source("y", "SELECT * FROM x")
    .source("z", "SELECT a FROM y")
    .schema("x", HashMap::from_iter([("a".into(), "int".into())]))
    .build();
```

This traces column `a` through:

1. The main query (`SELECT a FROM z`)
2. Source `z` (`SELECT a FROM y`)
3. Source `y` (`SELECT * FROM x`)
4. Down to table `x` with known schema

### Node Data Structure

Each node in the lineage tree contains:

| Field                 | Description                                |
| --------------------- | ------------------------------------------ |
| `name`                | Column or node identifier                  |
| `source`              | The source expression (full query context) |
| `expression`          | The specific expression for this column    |
| `downstream`          | Child nodes (dependencies)                 |
| `source_name`         | Name of the source table                   |
| `reference_node_name` | Referenced CTE name (if applicable)        |

## Supported SQL Features

| Feature                  | Status    |
| ------------------------ | --------- |
| Basic SELECT             | Supported |
| CTEs (WITH clause)       | Supported |
| Subqueries               | Supported |
| UNION / UNION ALL        | Supported |
| JOINs                    | Supported |
| SELECT \*                | Supported |
| VALUES clause            | Supported |
| UNNEST / array functions | Supported |
| Multiple dialects        | Supported |

## Limitations

- **No CLI command**: Lineage is not yet exposed as a sqruff CLI command
- **Snowflake LATERAL FLATTEN**: Not yet fully supported
- **DDL statements**: Not applicable (lineage focuses on SELECT queries)

## Example Output

For a query like:

```sql
WITH cte AS (SELECT a FROM source_table)
SELECT a FROM cte
```

The lineage tree shows:

```
name: a
source: with cte as (select source_table.a as a from source_table) select cte.a as a from cte
expression: cte.a as a
source_name:
reference_node_name:
└─ name: cte.a
   source: select source_table.a as a from source_table
   expression: source_table.a as a
   source_name:
   reference_node_name: cte
   └─ name: source_table.a
      source: source_table
      expression: source_table
      source_name: cte
      reference_node_name:
```
