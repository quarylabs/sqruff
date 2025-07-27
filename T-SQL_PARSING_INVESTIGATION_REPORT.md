# T-SQL Parsing Investigation Report

**Date:** 2025-07-27  
**Investigation:** Discrepancy between individual file parsing and unparsable.py script results  
**Scope:** 16 files reported as unparsable by the automated script  

## Executive Summary

**Key Finding:** All 16 files flagged as "unparsable" have **genuine parsing failures**, not just style/linting issues. The discrepancy occurs because individual testing may focus on specific syntax features that work, while these files contain additional T-SQL constructs that are not properly supported by the current parser.

**Impact:** These parsing failures prevent achieving 100% T-SQL parsing success and indicate gaps in the T-SQL dialect implementation.

## Investigation Methodology

1. **Automated Analysis:** Created scripts to systematically examine all 16 unparsable files
2. **Parsing Error Extraction:** Used `cargo run -- lint <file> --parsing-errors` to identify specific failures
3. **Syntax Analysis:** Examined the exact lines and syntax causing parsing issues
4. **Root Cause Categorization:** Grouped failures by underlying grammar/parser issues

## Detailed Findings

### 📊 Overall Statistics
- **Total unparsable files:** 16
- **Files with genuine parsing failures:** 16 (100%)
- **Files with only style issues:** 0 (0%)
- **Total parsing errors:** 27 specific "Unparsable section" errors
- **Root cause categories:** 10 distinct categories

### 🔍 File-by-File Analysis

| File | Parsing Errors | Primary Issue |
|------|----------------|---------------|
| `case_in_select.sql` | 2 | CASE expression parsing |
| `create_table_constraints.sql` | 1 | CREATE TABLE syntax |
| `create_table_with_sequence_bracketed.sql` | 1 | Sequence/Identity columns |
| `create_view.sql` | 1 | VIEW options (WITH CHECK OPTION) |
| `create_view_with_set_statements.sql` | 2 | CASE expressions in views |
| `join_hints.sql` | 2 | T-SQL join hints |
| `json_functions.sql` | 1 | JSON function syntax |
| `merge.sql` | 1 | MERGE OUTPUT clause |
| `nested_joins.sql` | 3 | Complex JOIN patterns |
| `openrowset.sql` | 3 | OPENROWSET function |
| `select.sql` | 3 | Multiple SELECT issues |
| `select_date_functions.sql` | 1 | DATEPART function |
| `table_object_references.sql` | 1 | Table reference patterns |
| `temporal_tables.sql` | 1 | Temporal table syntax |
| `triggers.sql` | 2 | Trigger-specific syntax |
| `update.sql` | 1 | UPDATE statement syntax |

## Root Cause Analysis

### 🔴 HIGH SEVERITY (Immediate Action Required)

#### 1. CASE Expression Parsing
- **Files affected:** 3 (`case_in_select.sql`, `select.sql`, `create_view_with_set_statements.sql`)
- **Issue:** CASE expressions in SELECT clauses not properly parsed
- **Examples:**
  ```sql
  CASE 
      WHEN Status = 'Active' THEN 'A'
      WHEN Status = 'Inactive' THEN 'I'
      ELSE 'U'
  END AS StatusCode
  ```
- **Impact:** CASE expressions are fundamental SQL constructs used in virtually all SQL dialects

#### 2. CREATE TABLE Syntax Issues  
- **Files affected:** 3 (`create_table_constraints.sql`, `create_table_with_sequence_bracketed.sql`, `temporal_tables.sql`)
- **Issue:** Various CREATE TABLE constructs not properly parsed
- **Examples:**
  ```sql
  CREATE TABLE [dbo].[example](
  CREATE TABLE SCHEMA_NAME.TABLE_NAME(
  ```
- **Impact:** Basic DDL statements must work for any SQL dialect implementation

### 🟡 MEDIUM SEVERITY (Important Features)

#### 3. T-SQL Join Hints
- **Files affected:** 1 (`join_hints.sql`)
- **Issue:** T-SQL specific join hints (HASH, MERGE, LOOP) not recognized
- **Examples:**
  ```sql
  FULL OUTER MERGE JOIN table2
  INNER HASH JOIN table2
  LEFT LOOP JOIN table2
  ```

#### 4. Advanced T-SQL Functions
- **Files affected:** 3 (`json_functions.sql`, `select_date_functions.sql`, `openrowset.sql`)
- **Issue:** Modern T-SQL functions not supported
- **Examples:**
  ```sql
  JSON_ARRAY('a', 1, NULL, 2, NULL ON NULL)
  DATEPART(day, [mydate], GETDATE())
  OPENROWSET(BULK 'path', FORMAT = 'PARQUET')
  ```

#### 5. Complex JOIN Patterns
- **Files affected:** 1 (`nested_joins.sql`)
- **Issue:** Multi-table joins with complex ON conditions
- **Examples:**
  ```sql
  LEFT OUTER JOIN (dbo.Test2 AS tst2
                    INNER JOIN dbo.FilterTable AS fltr1
  ```

#### 6. MERGE Statement Completeness
- **Files affected:** 1 (`merge.sql`)
- **Issue:** MERGE OUTPUT clause not supported
- **Examples:**
  ```sql
  OUTPUT deleted.*, $action, inserted.* INTO #MyTempTable
  ```

#### 7. Trigger Syntax
- **Files affected:** 1 (`triggers.sql`)
- **Issue:** Trigger-specific constructs not parsed
- **Examples:**
  ```sql
  FROM inserted AS i
  ON DATABASE
  ```

#### 8. Window Function Spacing
- **Files affected:** 1 (`select.sql`)
- **Issue:** Missing space between function and OVER clause
- **Examples:**
  ```sql
  ROW_NUMBER()OVER(PARTITION BY ...)  -- Missing space before OVER
  ```

### 🟢 LOW SEVERITY (Edge Cases)

#### 9. VIEW Options
- **Files affected:** 1 (`create_view.sql`)
- **Issue:** `WITH CHECK OPTION` not supported
- **Examples:**
  ```sql
  WITH CHECK OPTION ;
  ```

#### 10. Table Reference Edge Cases
- **Files affected:** 2 (`table_object_references.sql`, `update.sql`)
- **Issue:** Unusual table reference patterns
- **Examples:**
  ```sql
  select column_1 from .[#my_table];
  UPDATE stuff SET
  ```

## Why the Discrepancy Exists

The discrepancy between individual file testing and the unparsable.py script results occurs because:

1. **Partial Parsing Success:** Individual syntax features may work in isolation, but these files contain combinations of syntax that include unsupported constructs
2. **Test Coverage Gaps:** Individual testing may focus on specific features that are implemented, missing the problematic syntax
3. **Complex File Content:** These test files contain multiple T-SQL constructs, and any single unsupported feature causes the entire file to be marked as having parsing failures

## Recommendations

### 🎯 Immediate Actions (HIGH Priority)
1. **Fix CASE expression parsing** - This affects 3 files and is a fundamental SQL construct
2. **Resolve CREATE TABLE issues** - Basic DDL statements must work properly
3. **Address SELECT parsing problems** - Core query functionality must be solid

### 🔧 Implementation Actions (MEDIUM Priority)
1. **Add T-SQL join hint support** - Extend join grammar to include HASH, MERGE, LOOP hints
2. **Implement missing T-SQL functions** - Add JSON_ARRAY, enhanced DATEPART, OPENROWSET
3. **Fix complex JOIN parsing** - Handle nested and parenthesized JOIN patterns
4. **Complete MERGE statement support** - Add OUTPUT clause functionality
5. **Add trigger syntax support** - Handle trigger-specific constructs

### 📋 Enhancement Actions (LOW Priority)
1. **Add VIEW options** - Support WITH CHECK OPTION
2. **Handle edge case patterns** - Address unusual table reference syntax
3. **Fix window function spacing** - Allow ROW_NUMBER()OVER patterns

## Technical Implementation Areas

Based on the analysis, the following areas in the codebase likely need attention:

1. **`crates/lib-dialects/src/tsql.rs`** - Main T-SQL dialect grammar
2. **CASE expression grammar** - Expression parsing rules
3. **CREATE TABLE grammar** - DDL statement parsing
4. **JOIN hint grammar** - JOIN clause extensions
5. **Function parsing** - T-SQL specific function support

## Success Metrics

After implementing fixes, success can be measured by:

1. **Re-running `unparsable.py`** - Target: 0 unparsable files
2. **Parsing error reduction** - Target: 0 "Unparsable section" errors in these 16 files
3. **Functionality preservation** - Ensure existing tests continue to pass
4. **Comprehensive testing** - Add test cases for fixed syntax patterns

## Conclusion

The investigation reveals that all 16 "unparsable" files have legitimate parsing failures that prevent 100% T-SQL parsing success. The issues range from fundamental SQL constructs (CASE expressions, CREATE TABLE) to T-SQL specific features (join hints, advanced functions). 

**Priority focus should be on HIGH severity issues** (CASE expressions and CREATE TABLE syntax) as these affect core SQL functionality that users expect to work reliably.

Addressing these parsing failures systematically will move Sqruff significantly closer to complete T-SQL compatibility and eliminate the discrepancy between individual feature testing and comprehensive file parsing.