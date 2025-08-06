# RF01 Column Alias Bug Investigation

## Issue Summary
RF01 rule incorrectly flags valid MERGE statement references when EXISTS subquery contains column aliases.

## Test Case
- **Test**: `test_pass_postgres_merge_with_alias` in RF01.yml
- **Dialect**: T-SQL (despite "postgres" in name)
- **Status**: Currently ignored

## Problem Statement
```sql
MERGE dw.sch.tbl dest
USING land.sch.tbl src
ON src.idx = dest.idx
WHEN NOT MATCHED BY SOURCE AND EXISTS (
    SELECT 1 AS tmp  -- Column alias causes the bug
    FROM land.sch.tag AS ld
    WHERE ld.idx = dest.idx  -- RF01 incorrectly flags dest.idx as not found
)
THEN UPDATE SET dest.ac = 'N'
```

## Investigation Steps

### 1. Reproduce the Issue
- [x] Confirmed issue with original SQL
- [x] Confirmed SQL works without `AS tmp` alias
- [x] Isolated that column alias in EXISTS is the trigger

### 2. Test Across Dialects
Testing the same pattern in different dialects to see if this is T-SQL specific.

#### Dialect Support for MERGE
- **T-SQL**: Supports `WHEN NOT MATCHED BY SOURCE` - FAILS with column alias in EXISTS
- **Snowflake**: Uses `MERGE INTO`, supports `WHEN MATCHED` - PASSES with column alias
- **BigQuery**: Uses `MERGE`, supports `WHEN MATCHED` - PASSES with column alias  
- **ANSI**: Doesn't support the tested MERGE syntax
- **Postgres**: Doesn't support the tested MERGE syntax

#### Test Results

##### T-SQL MERGE with EXISTS and column alias
```sql
-- FAILS: RF01 flags dest.idx as not found
MERGE dw.sch.tbl dest
USING land.sch.tbl src
ON src.idx = dest.idx
WHEN NOT MATCHED BY SOURCE AND EXISTS (
    SELECT 1 AS tmp  -- Column alias triggers bug
    FROM land.sch.tag AS ld
    WHERE ld.idx = dest.idx
)
THEN UPDATE SET dest.ac = 'N';
```

##### T-SQL MERGE without column alias
```sql
-- PASSES: Works fine without AS tmp
MERGE dw.sch.tbl dest
USING land.sch.tbl src
ON src.idx = dest.idx
WHEN NOT MATCHED BY SOURCE AND EXISTS (
    SELECT 1  -- No alias, no problem
    FROM land.sch.tag AS ld
    WHERE ld.idx = dest.idx
)
THEN UPDATE SET dest.ac = 'N';
```

##### T-SQL regular SELECT with EXISTS and column alias
```sql
-- PASSES: Regular SELECT works fine with column alias
SELECT *
FROM table1 AS t1
WHERE EXISTS (
    SELECT 1 AS tmp
    FROM table2 AS t2
    WHERE t2.id = t1.id
);
```

##### Snowflake/BigQuery MERGE with EXISTS and column alias
```sql
-- PASSES in both Snowflake and BigQuery
MERGE INTO schema1.table1 dest
USING schema2.table2 src
ON src.idx = dest.idx
WHEN MATCHED AND EXISTS (
    SELECT 1 AS tmp
    FROM schema3.table3 AS ld
    WHERE ld.idx = dest.idx
)
THEN UPDATE SET dest.status = 'DELETED';
```

### 3. Key Findings
1. **Issue is T-SQL specific**: Snowflake and BigQuery handle column aliases in EXISTS correctly
2. **Issue is MERGE specific**: Regular SELECT statements work fine in T-SQL
3. **Trigger is column alias**: Removing `AS tmp` fixes the issue
4. **Scope is `WHEN NOT MATCHED BY SOURCE`**: The specific MERGE clause where it fails

### 4. Code Analysis

#### RF01 Rule Implementation
- Located in `/crates/lib/src/rules/references/rf01.rs`
- Handles MergeStatement in `crawl_behaviour()`
- Uses `Query::from_segment()` to build query structure
- Calls `analyze_table_references()` to check references

#### Query Building Process
1. `Query::from_segment()` in `/crates/lib-core/src/utils/analysis/query.rs`
2. For MERGE statements:
   - Added to selectables as `SUBSELECT_TYPES` contains `MergeStatement`
   - Subqueries extracted via `extract_subqueries()`
3. `Selectable::select_info()` returns None for MERGE (no SelectClause)

#### Hypothesis
The bug appears when:
1. MERGE statement creates a selectable with no select_info (returns None)
2. EXISTS subquery with column alias (`AS tmp`) is processed
3. The column alias somehow interferes with parent scope resolution
4. When RF01 tries to resolve `dest.idx`, it can't find the parent MERGE aliases

The issue is likely in how `analyze_table_references()` handles selectables that return None for `select_info()` when there are subqueries with column aliases.

### 5. Root Cause Investigation

#### The Core Problem
1. **MERGE aliases not extracted**: The function `get_aliases_from_select()` only looks for aliases in FROM clauses
2. **MERGE has different structure**: 
   - Target alias: `MERGE table_name alias`
   - Source alias: `USING table_name alias`
   - No FROM clause exists
3. **Result**: When `select_info()` is called on MERGE, it returns None (no aliases found)

#### Why Column Alias Triggers the Bug
When the EXISTS subquery contains `SELECT 1 AS tmp`:
1. The subquery becomes a selectable with its own context
2. RF01 tries to resolve references within this subquery
3. Since the parent MERGE has no extracted aliases (select_info returns None)
4. The reference resolution fails to find `dest` in parent scope
5. Without the column alias, the subquery processing is different (simpler)

#### Dialect Differences
- **Snowflake/BigQuery**: Use `MERGE INTO` syntax, may have different alias extraction
- **T-SQL**: Uses `MERGE` syntax, relies on standard FROM clause alias extraction

### 6. Solution Approach
To fix this issue, we need to:
1. Create MERGE-specific alias extraction that recognizes target and source aliases
2. Modify `get_aliases_from_select()` or create a new function for MERGE statements
3. Extract aliases from:
   - Target table: `MERGE schema.table alias`
   - Source table: `USING schema.table alias`
4. Ensure these aliases are available in the query scope for reference resolution

### 7. Final Conclusions

#### Root Cause Summary
1. **Primary Issue**: The `get_aliases_from_select()` function only extracts aliases from FROM clauses
2. **MERGE Structure**: MERGE statements don't have FROM clauses; they have target and source tables
3. **Missing Implementation**: No MERGE-specific alias extraction exists
4. **Trigger**: Column aliases in EXISTS subqueries create a more complex scope resolution scenario that exposes this gap

#### Why Other Dialects Work
- **Snowflake/BigQuery**: Use `MERGE INTO` syntax and may have different processing paths
- **Test Coverage**: The specific combination of MERGE + EXISTS + column alias may not be tested in other dialects
- **Implementation Differences**: Other dialects might handle MERGE aliases differently

#### Impact
- Affects T-SQL MERGE statements with subqueries containing column aliases
- RF01 rule incorrectly flags valid references as "not found in FROM clause"
- Workaround: Remove column aliases from subqueries in MERGE statements

#### Recommended Fix
Implement proper MERGE statement alias extraction in the Query/Selectable infrastructure to ensure target and source table aliases are available for reference resolution in all nested contexts.