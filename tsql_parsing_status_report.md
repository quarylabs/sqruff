# T-SQL Parsing Status Report

## Summary

After recent fixes to T-SQL parsing, 17 out of 153 test files (11%) still contain unparsable sections. The recent fixes successfully addressed:
- PRIMARY KEY constraints in DECLARE TABLE
- WITHIN GROUP syntax for aggregate functions  
- IGNORE NULLS in window functions
- Named windows in OVER clauses

## Files with Remaining Issues

### Critical Issues (Multiple unparsable sections or complex patterns)

1. **temporal_tables.yml** - Complex CREATE TABLE statements with advanced features
2. **create_table_with_sequence_bracketed.yml** - Long multi-statement sequences
3. **triggers.yml** - Database-level triggers and complex trigger syntax

### Moderate Issues (Specific syntax patterns)

4. **openrowset.yml** - Table alias "AS rows" not parsing correctly
5. **merge.yml** - MERGE statement "WHEN NOT MATCHED BY SOURCE" clause
6. **json_functions.yml** - JSON "ON NULL" clause
7. **join_hints.yml** - Complex join syntax (FULL OUTER MERGE JOIN, LEFT LOOP JOIN)
8. **select.yml** - Complex CASE expressions in SELECT clause
9. **create_view.yml** - WITH CHECK OPTION clause
10. **update.yml** - UPDATE with OUTPUT clause

### Minor Issues (Single patterns)

11. **create_table_constraints.yml** - Complex column constraints
12. **create_table.yml** - Table creation with specific constraints
13. **set_statements.yml** - NEXT VALUE FOR sequence operations
14. **select_date_functions.yml** - DATEPART function parsing
15. **create_view_with_set_statements.yml** - CASE expressions in views
16. **nested_joins.yml** - Complex nested join conditions
17. **table_object_references.yml** - Table reference parsing issues

## Top Priority Patterns to Fix

### 1. CASE Expressions (3+ occurrences)
**Pattern**: CASE expressions not fully parsing in SELECT clauses and views
**Example**: 
```sql
SELECT CASE 
    WHEN condition THEN result 
    ELSE 'default' 
END
```
**Files affected**: select.yml, create_view_with_set_statements.yml

### 2. Complex JOIN Syntax (3 occurrences)
**Pattern**: Advanced join hints and types not parsing
**Examples**:
- `FULL OUTER MERGE JOIN`
- `LEFT LOOP JOIN`
- `INNER HASH JOIN`
**Files affected**: join_hints.yml, nested_joins.yml

### 3. MERGE Statement Clauses (1 occurrence)
**Pattern**: `WHEN NOT MATCHED BY SOURCE` clause in MERGE statements
**Example**:
```sql
MERGE ... 
WHEN NOT MATCHED BY SOURCE AND condition 
THEN UPDATE SET ...
```
**Files affected**: merge.yml

### 4. Table Alias "AS rows" (2 occurrences)
**Pattern**: Special case where 'rows' is used as a table alias after OPENROWSET
**Example**:
```sql
SELECT * FROM OPENROWSET(...) AS rows
```
**Files affected**: openrowset.yml

### 5. WITH CHECK OPTION (1 occurrence)
**Pattern**: View creation with CHECK OPTION
**Example**:
```sql
CREATE VIEW ... WITH CHECK OPTION
```
**Files affected**: create_view.yml

### 6. Advanced CREATE TABLE Features
**Pattern**: Complex table creation with:
- SYSTEM_VERSIONING
- LEDGER tables
- REMOTE_DATA_ARCHIVE
- Multiple table options in WITH clause
**Files affected**: temporal_tables.yml, create_table_constraints.yml

### 7. Database-Level Triggers
**Pattern**: Triggers on DATABASE or ALL SERVER
**Example**:
```sql
CREATE TRIGGER ... ON DATABASE
CREATE TRIGGER ... ON ALL SERVER
```
**Files affected**: triggers.yml

### 8. SEQUENCE Operations
**Pattern**: NEXT VALUE FOR sequence syntax
**Example**:
```sql
DEFAULT (NEXT VALUE FOR [dbo].[sequence_name])
```
**Files affected**: create_table_with_sequence_bracketed.yml, set_statements.yml

## Recommendations

1. **Priority 1**: Fix CASE expressions as they appear in multiple contexts
2. **Priority 2**: Fix complex JOIN syntax (HASH/MERGE/LOOP join hints)
3. **Priority 3**: Fix MERGE statement clauses
4. **Priority 4**: Fix special table aliases like "AS rows"
5. **Priority 5**: Fix advanced CREATE TABLE options for modern T-SQL features

These fixes would resolve the majority of unparsable sections and significantly improve T-SQL dialect support.