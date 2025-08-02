# T-SQL Parsing Issues Review

## Executive Summary

This document provides a comprehensive review of T-SQL parsing issues found in the sqruff dialect test fixtures. Unlike unparsable sections (which are completely unrecognized), these issues involve SQL constructs that parse but map to incorrect AST node types, leading to semantic incorrectness.

The review analyzed **89 T-SQL test fixture files** and identified multiple categories of parsing issues that need correction for proper T-SQL dialect support.

## Critical Issues by Category

### 1. Control Flow Statement Issues

#### IF-ELSE Statement Parsing (HIGH PRIORITY)
**Files affected:** `if_else.yml`, `if_else_begin_end.yml`

**Issue:** ELSE clauses are incorrectly parsed as `naked_identifier` instead of being part of the `if_statement` construct.

**Current incorrect parsing:**
```yaml
- statement:
  - naked_identifier: ELSE  # ❌ WRONG: Should be part of if_statement
  - expression:
    - select_statement: ...
```

**Expected correct parsing:**
```yaml
- if_statement:
  - keyword: IF
  - expression: ...
  - statement: ...  # IF body
  - else_clause:    # ✅ CORRECT
    - keyword: ELSE
    - statement: ... # ELSE body
```

**Impact:** This breaks linting rules that depend on proper IF-ELSE structure recognition and affects code formatting.

#### WHILE Statement Parsing (MEDIUM PRIORITY)
**Files affected:** `while_statement.yml`

**Issue:** WHILE statements are parsed as generic statements rather than specific `while_statement` constructs.

**Current parsing:**
```yaml
- statement:
  - keyword: WHILE  # ❌ Missing while_statement wrapper
  - expression: ...
  - statement: ...
```

#### TRY-CATCH Block Parsing (HIGH PRIORITY)
**Files affected:** `try_catch.yml`

**Issue:** TRY-CATCH blocks are incorrectly parsed, with parts falling into unparsable sections.

### 2. DELETE Statement Issues (CRITICAL)

#### JOIN Clause Misinterpretation (CRITICAL)
**Files affected:** `delete.yml` (lines 280-396)

**Issue:** DELETE statements with FROM clauses containing JOINs are severely misparsed. The FROM clause and subsequent JOIN are parsed as separate statements rather than part of the DELETE.

**Current incorrect parsing:**
```yaml
- delete_statement:
  - keyword: DELETE
  - alias_expression:
    - naked_identifier: FROM  # ❌ WRONG: FROM treated as alias
- statement:  # ❌ WRONG: JOIN parsed as separate statement
  - naked_identifier: Sales
  - expression:
    - data_type:
      - data_type_identifier: INNER  # ❌ WRONG: INNER JOIN as data type
```

**Expected structure:**
```yaml
- delete_statement:
  - keyword: DELETE
  - table_reference: spqh
  - from_clause:           # ✅ CORRECT
    - keyword: FROM
    - from_expression:
      - join_clause:       # ✅ CORRECT
        - keyword: INNER
        - keyword: JOIN
```

### 3. EXECUTE Statement Issues (MEDIUM PRIORITY)

#### Mixed Statement Recognition
**Files affected:** `execute.yml`

**Issue:** Some EXECUTE statements parse correctly while others have components parsed as separate expressions or statements.

**Examples of misidentification:**
- Line 12-13: `EXECUTE` parsed as `column_reference` instead of keyword
- Line 184-194: EXEC statement fragments parsed as individual expressions

### 4. Statement Boundary Issues (HIGH PRIORITY)

#### Missing Statement Wrappers
**Files affected:** Multiple files

**Issue:** Many T-SQL constructs that should be wrapped in specific statement types are parsed as generic statements.

**Examples:**
- `PRINT` statements (delete.yml:62-66)
- `BREAK` statements (while_statement.yml:56-58)
- Variable assignments with SET

#### CREATE PROCEDURE Complexity
**Files affected:** `create_procedure.yml`, `function_no_return.yml`

**Issue:** Complex stored procedure bodies with IF statements, loops, and variable assignments have parsing issues within the procedure body.

### 5. T-SQL Specific Syntax Issues

#### Assignment Operators (MEDIUM PRIORITY)
**Files affected:** `set_statements.yml`

**Issue:** T-SQL compound assignment operators (+=, -=, *=, /=, %=, ^=, &=, |=) have inconsistent parsing.

**Current mixed parsing:**
```yaml
- assignment_operator:
  - addition_assignment_segment: +=  # ✅ CORRECT for +=, -=
- assignment_operator:
  - assignment_operator:             # ❌ WRONG: Double wrapping
    - binary_operator: ^
    - raw_comparison_operator: =
```

#### Variable and Parameter Handling
**Files affected:** Multiple procedure and function files

**Issue:** T-SQL variables (@variable) and parameters sometimes parsed correctly, other times as expressions or column references.

#### OUTPUT Clause Parsing
**Files affected:** `delete.yml`, `update.yml`, `merge.yml`

**Issue:** OUTPUT clauses in DML statements generally parse correctly, but the SELECT elements within them sometimes have node type issues.

## Parsing Quality Assessment

### Well-Parsed Constructs ✅
- Basic SELECT, INSERT, UPDATE statements
- MERGE statements (complex but mostly correct)
- CREATE TABLE and basic DDL
- Common Table Expressions (CTEs)
- Most function calls and expressions
- DECLARE statements for variables and cursors
- BEGIN/END blocks (mostly)

### Problematic Constructs ❌
- Control flow statements (IF-ELSE, WHILE, TRY-CATCH)
- DELETE with JOINs
- Complex stored procedure bodies
- Statement boundary recognition
- Some EXECUTE statement variants

## Recommendations

### Immediate Actions (Critical)
1. **Fix DELETE statement FROM clause parsing** - This is a critical issue affecting basic DML operations
2. **Implement proper IF-ELSE statement structure** - Essential for control flow recognition
3. **Review statement boundary detection** - Many constructs need proper statement wrapper nodes

### Medium-term Improvements
1. **Standardize T-SQL assignment operator parsing** - Ensure consistent AST structure
2. **Improve EXECUTE statement recognition** - Handle all variants consistently
3. **Enhance procedure/function body parsing** - Better handling of complex control structures

### Long-term Goals
1. **Implement comprehensive T-SQL control flow grammar** - WHILE, TRY-CATCH, etc.
2. **Review all T-SQL specific syntax** - Ensure proper node type mapping
3. **Add more complex test cases** - Test edge cases and combinations

## Testing Strategy

### Priority Test Cases
1. DELETE statements with various JOIN types
2. IF-ELSE statements with nested structures
3. Complex stored procedures with mixed control flow
4. EXECUTE statements with different parameter styles

### Regression Prevention
1. Ensure current working constructs remain functional
2. Add negative test cases for known problematic patterns
3. Validate that fixes don't break well-parsing constructs

## Test Suite Maintenance Issues

### Duplicate Test Files (CRITICAL)
**Issue:** Found **exact duplicate test files** that serve no testing purpose and clutter the test suite.

**Duplicates identified:**
- `with_result_sets_debug.yml` and `with_result_sets_simple.yml` (MD5: 64cb15c8030074dc28afc3c4c2540d92)

**Recommendation:** Remove one of the duplicate files to clean up the test suite.

### AST Node Duplication Issues (CRITICAL)
**Issue:** **Systematic over-nesting** of AST nodes throughout the T-SQL test fixtures, indicating fundamental parsing grammar issues.

## Comprehensive Duplication Pattern Analysis

### 1. Statement-Level Duplications (CRITICAL - WRONG)

**Pattern:** Double `statement` nesting at the root level
**SQL Context:** Every T-SQL statement (SELECT, DELETE, CREATE, etc.)
**Files Affected:** ALL T-SQL test files (159 files)

```yaml
- statement:
  - statement:  # ❌ WRONG: Redundant wrapper
    - select_statement:
```

**Analysis:** This is **definitely incorrect**. SQL statements should not be double-wrapped. The pattern shows:
- Line 2: `- statement:` (file-level statement container)
- Line 3: `  - statement:` (redundant intermediate wrapper)
- Line 4: `    - select_statement:` (actual statement type)

**Expected Structure:**
```yaml
- statement:  # ✅ CORRECT: Single statement wrapper
  - select_statement:
    - select_clause:
```

### 2. FROM Clause Duplications (CRITICAL - WRONG)

**Pattern:** Multiple levels of `from_expression` and `from_expression_element` nesting
**SQL Context:** Any FROM clause with tables or JOINs
**Files Affected:** `hints.yml`, `al05_exact_issue.yml`, most SELECT statements

```sql
-- SQL: Simple table reference
FROM schema1.Table_Sales_Position_Reference AS op2ref
```

```yaml
# ❌ CURRENT WRONG PARSING:
- from_expression:
  - from_expression:          # WRONG: Redundant level
    - from_expression_element:
      - from_expression_element:  # WRONG: Redundant level
        - table_expression:
```

**Analysis:** This creates **4 levels of nesting** where only 2 should exist. Each table reference gets double-wrapped.

**Expected Structure:**
```yaml
# ✅ CORRECT PARSING:
- from_expression:
  - from_expression_element:
    - table_expression:
      - table_reference:
```

### 3. Object Reference Duplications (CRITICAL - WRONG)

**Pattern:** Double `object_reference` nesting for schema.table references
**SQL Context:** Any multi-part identifier (schema.table, database.schema.table)
**Files Affected:** Most files with qualified table names

```sql
-- SQL: Schema-qualified table
schema1.Table_Sales_Position_Reference
Sales.Customer
dbo.ISOweek
```

```yaml
# ❌ CURRENT WRONG PARSING:
- object_reference:
  - object_reference:  # WRONG: Redundant level
    - naked_identifier: schema1
    - dot: .
    - naked_identifier: Table_Sales_Position_Reference
```

**Analysis:** Multi-part identifiers should be represented as a **single object_reference** with multiple components, not nested object_references.

**Expected Structure:**
```yaml
# ✅ CORRECT PARSING:
- object_reference:
  - naked_identifier: schema1
  - dot: .
  - naked_identifier: Table_Sales_Position_Reference
```

### 4. SELECT Clause Duplications (CRITICAL - WRONG)

**Pattern:** Double `select_clause` nesting
**SQL Context:** All SELECT statements
**Files Affected:** All files with SELECT statements

```sql
-- SQL: Basic SELECT
SELECT COUNT(*)
```

```yaml
# ❌ CURRENT WRONG PARSING:
- select_clause:
  - select_clause:  # WRONG: Redundant level
    - keyword: SELECT
    - select_clause_element:
```

**Analysis:** SELECT clauses should not be double-nested. This is clearly wrong.

**Expected Structure:**
```yaml
# ✅ CORRECT PARSING:
- select_clause:
  - keyword: SELECT
  - select_clause_element:
```

### 5. Function Call Duplications (CRITICAL - WRONG)

**Pattern:** Double `function` nesting
**SQL Context:** Function calls like COUNT(*), DATEPART(), CAST()
**Files Affected:** `al05_exact_issue.yml`, `create_function.yml`

```sql
-- SQL: Function call
COUNT(*)
DATEPART(wk, @DATE)
```

```yaml
# ❌ CURRENT WRONG PARSING:
- function:
  - function:  # WRONG: Redundant level
    - function_name:
      - function_name_identifier: COUNT
```

**Analysis:** Function calls should not be double-nested. This adds unnecessary complexity.

**Expected Structure:**
```yaml
# ✅ CORRECT PARSING:
- function:
  - function_name:
    - function_name_identifier: COUNT
  - bracketed:
```

### 6. Data Type Duplications (MEDIUM PRIORITY - CONTEXT DEPENDENT)

**Pattern:** Double `data_type` nesting
**SQL Context:** Complex data types in function parameters and CAST expressions
**Files Affected:** `create_function.yml`

```sql
-- SQL: CAST with data type
CAST(DATEPART(yy,@DATE) as CHAR(4))
```

```yaml
# ❌ POTENTIALLY WRONG:
- data_type:
  - data_type:  # QUESTIONABLE: May indicate CAST vs declaration difference
    - data_type_identifier: CHAR
```

**Analysis:** This may be context-dependent. Different grammar rules for:
- Function parameter declarations: `@param CHAR(4)`
- CAST expressions: `CAST(value AS CHAR(4))`

### 7. Statement Type Duplications (CRITICAL - WRONG)

**Pattern:** Double statement type nesting (delete_statement, create_function_statement)
**SQL Context:** DDL and DML statements
**Files Affected:** `delete.yml`, `create_function.yml`

```sql
-- SQL: DELETE statement
DELETE FROM Sales.SalesPersonQuotaHistory;
```

```yaml
# ❌ CURRENT WRONG PARSING:
- delete_statement:
  - delete_statement:  # WRONG: Redundant level
    - keyword: DELETE
```

**Analysis:** Statement types should never be double-nested. This is clearly incorrect.

**Expected Structure:**
```yaml
# ✅ CORRECT PARSING:
- delete_statement:
  - keyword: DELETE
  - keyword: FROM
  - table_reference:
```

## Impact Assessment

### Critical Issues (WRONG - Fix Immediately)
1. **statement** duplications - affects ALL files
2. **from_expression/from_expression_element** duplications - breaks JOIN parsing
3. **object_reference** duplications - affects qualified names
4. **select_clause** duplications - affects all SELECT statements
5. **function** duplications - affects function calls
6. **[statement_type]** duplications - affects DML/DDL statements

### Grammar Rule Issues Identified
- **Root statement wrapper** creating unnecessary nesting
- **FROM clause grammar** over-complicating table references
- **Object reference grammar** double-wrapping qualified names
- **Function call grammar** adding redundant function node
- **Statement type grammars** self-nesting inappropriately

**Files heavily affected:**
- `al05_exact_issue.yml` - Multiple duplication patterns
- `hints.yml` - Extensive `from_expression_element` duplications (20+ instances)
- `delete.yml` - Statement and object reference duplications
- `create_function.yml` - Function, data type, and statement duplications
- **ALL 159 T-SQL test files** - statement-level duplications

**Root Cause:** T-SQL dialect grammar rules are creating unnecessary intermediate nodes that should be flattened. The grammar likely has recursive rules that are incorrectly self-referencing.

## Files Requiring Immediate Attention

1. **`delete.yml`** (lines 280-396) - Critical JOIN parsing issues
2. **`if_else.yml`** - ELSE clause recognition
3. **`if_else_begin_end.yml`** - Control flow structure
4. **`execute.yml`** - Mixed statement recognition issues
5. **`function_no_return.yml`** - Unparsable procedure bodies
6. **`with_result_sets_debug.yml` OR `with_result_sets_simple.yml`** - Remove duplicate

## Conclusion

The T-SQL dialect parsing has a solid foundation for basic constructs but requires significant work on:
- Control flow statements
- Complex DML operations (especially DELETE with JOINs)
- Statement boundary recognition
- T-SQL specific syntax consistency

Addressing the critical issues identified above will significantly improve T-SQL dialect support and enable proper linting rule functionality.