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

## Files Requiring Immediate Attention

1. **`delete.yml`** (lines 280-396) - Critical JOIN parsing issues
2. **`if_else.yml`** - ELSE clause recognition
3. **`if_else_begin_end.yml`** - Control flow structure
4. **`execute.yml`** - Mixed statement recognition issues
5. **`function_no_return.yml`** - Unparsable procedure bodies

## Conclusion

The T-SQL dialect parsing has a solid foundation for basic constructs but requires significant work on:
- Control flow statements
- Complex DML operations (especially DELETE with JOINs)
- Statement boundary recognition
- T-SQL specific syntax consistency

Addressing the critical issues identified above will significantly improve T-SQL dialect support and enable proper linting rule functionality.