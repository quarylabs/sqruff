# T-SQL Dialect Test Expectation Files - Detailed Analysis Report

## Executive Summary

This report analyzes the T-SQL dialect test expectation files in `crates/lib-dialects/test/fixtures/dialects/tsql/*.yml` to identify unusual patterns, duplications, and potential parsing issues. The analysis reveals several categories of problems that suggest opportunities for parser improvements and AST structure optimization.

## Methodology

- Reviewed 90+ T-SQL dialect test expectation YAML files
- Analyzed AST structure patterns for redundancy and inconsistencies  
- Identified parent-child type duplications and malformed structures
- Categorized findings by severity and type

## Key Findings

### 1. Redundant Parent-Child Type Duplications

#### 1.1 Double-Wrapped Statement Types

**File: `create_function.yml`**
```yaml
- statement:
  - create_function_statement:    # ← Parent
    - create_function_statement:  # ← Child (identical type)
      - keyword: CREATE
      - keyword: FUNCTION
      # ... rest of structure
```

**Impact**: Creates unnecessary AST depth and complicates traversal logic.

**Affected Files**: 
- `create_function.yml` (lines 4-6)
- `create_table.yml` (multiple instances)

#### 1.2 Function Nesting Redundancy

**File: `convert.yml`**
```yaml
- function:           # ← Parent
  - function:         # ← Child (identical type)
    - function_name:
      - function_name_identifier: CONVERT
    - function_contents:
      # ... function parameters
```

**Pattern Frequency**: Occurs in every function call within the file
**Impact**: Doubles memory usage for function representations

### 2. Parser Failure Patterns

#### 2.1 Token-Level Fallback Parsing

**File: `stored_procedure_begin_end.yml`**

Instead of structured AST:
```yaml
- create_procedure_statement:
  - keyword: CREATE
  - keyword: PROCEDURE
  - object_reference: ...
```

Parser produces flat token sequence:
```yaml
- word: CREATE
- word: PROCEDURE  
- word: dbo
- dot: .
- word: Test_Begin_End
- word: AS
- word: BEGIN
# ... continues as individual tokens
```

**Root Cause**: Parser fails to recognize CREATE PROCEDURE pattern, falling back to token-by-token parsing
**Impact**: Loss of semantic structure, making rule application impossible

#### 2.2 Mixed Parsing Quality

**File: `if_else.yml`**

First part parsed correctly:
```yaml
- if_statement:
  - keyword: IF
  - numeric_literal: '1'
  - comparison_operator:
    - raw_comparison_operator: <
    - raw_comparison_operator: =
  - expression:
    - bracketed:
      - start_bracket: (
      - expression:
        - select_statement:  # ← Properly structured
          - select_clause:
            - keyword: SELECT
            # ... proper AST structure
```

ELSE clause degrades to tokens:
```yaml
- keyword: ELSE
- word: SELECT        # ← Should be part of select_statement
- word: ProductKey
- comma: ','
- word: EnglishDescription
# ... continues as flat tokens
```

**Impact**: Inconsistent behavior within single SQL statement makes rule implementation complex

### 3. Malformed Object References

#### 3.1 Leading Dot Issues

**File: `table_object_references.yml`**

```yaml
- object_reference:
  - dot: .                    # ← Malformed: leading dot without identifier
  - quoted_identifier: '"#my_table"'
```

**Expected Structure**:
```yaml
- object_reference:
  - naked_identifier: database_name
  - dot: .
  - naked_identifier: schema_name  
  - dot: .
  - quoted_identifier: '"#my_table"'
```

#### 3.2 Multiple Consecutive Dots

**File: `table_object_references.yml` (lines 52, 71)**

```yaml
- object_reference:
  - dot: .
  - dot: .
  - quoted_identifier: '[#my_table]'
```

And even:
```yaml
- object_reference:
  - dot: .
  - dot: .
  - dot: .
  - quoted_identifier: '[#my_table]'
```

**Impact**: Creates invalid object reference structures that don't correspond to valid T-SQL syntax

### 4. Data Type Parsing Errors

#### 4.1 Computed Column Misidentification

**File: `create_table.yml` (lines 243-252)**

```yaml
- data_type:
  - data_type:
    - data_type_identifier: AS    # ← Incorrect: AS is not a data type
    - bracketed:
      - start_bracket: (
      - expression:
        - column_reference:
          - naked_identifier: QtySold
        - binary_operator: '*'
        - column_reference:
          - naked_identifier: UnitPrice
      - end_bracket: )
```

**Expected Structure**:
```yaml
- computed_column_definition:
  - naked_identifier: SoldValue
  - keyword: AS
  - expression:
    - bracketed:
      - start_bracket: (
      - expression:
        - column_reference:
          - naked_identifier: QtySold
        - binary_operator: '*'
        - column_reference:
          - naked_identifier: UnitPrice
      - end_bracket: )
```

### 5. Inconsistent Column Definition Wrapping

**File: `create_table.yml`**

Some column definitions are double-wrapped:
```yaml
- column_definition:
  - column_definition:    # ← Redundant wrapper
    - naked_identifier: ProductID
    - data_type:
      - data_type:        # ← Another redundant wrapper
        - data_type_identifier: int
```

While others are single-wrapped:
```yaml
- column_definition:
  - naked_identifier: InventoryTs
  - word: datetime2       # ← Also inconsistent: should be data_type
  - bracketed:
    - start_bracket: (
    - numeric_literal: '0'
    - end_bracket: )
```

## Impact Analysis

### High Impact Issues

1. **Parser Failures** (Critical)
   - Complete loss of semantic structure
   - Rules cannot be applied to unparsed content
   - Affects multiple complex statement types

2. **Malformed Object References** (High)
   - Invalid AST structures that don't represent valid SQL
   - Could cause crashes in rule implementations
   - Affects fundamental SQL parsing

### Medium Impact Issues

3. **Double-Wrapped Nodes** (Medium)
   - Increases memory usage
   - Complicates AST traversal logic
   - Makes rule writing more complex

4. **Data Type Parsing Errors** (Medium)
   - Incorrect semantic representation
   - Could lead to false positive/negative rule violations
   - Affects DDL statement understanding

### Low Impact Issues

5. **Inconsistent Wrapping** (Low)
   - Makes rule implementations more complex
   - Reduces code maintainability
   - Inconsistent developer experience

## Recommendations

### Immediate Actions (High Priority)

1. **Fix Parser Failures**
   - Investigate why CREATE PROCEDURE statements fall back to token parsing
   - Ensure IF-ELSE statements parse consistently throughout
   - Add proper grammar rules for unparsed constructs

2. **Resolve Malformed Object References**
   - Fix leading dot parsing in object references
   - Prevent multiple consecutive dots without identifiers
   - Validate object reference structure in tests

### Medium-Term Improvements

3. **Eliminate Redundant Wrappers**
   - Remove double-wrapped `create_function_statement` nodes
   - Simplify function call AST structure
   - Standardize column definition wrapping

4. **Fix Data Type Recognition**
   - Properly parse computed column definitions
   - Ensure data types are consistently wrapped in `data_type` nodes
   - Add validation for data type identifier validity

### Long-Term Optimizations

5. **AST Structure Consistency**
   - Establish consistent wrapping patterns across all node types
   - Create AST validation rules to prevent malformed structures
   - Implement automated checks for AST quality in tests

## Testing Recommendations

1. **Add Negative Test Cases**
   - Test malformed SQL to ensure graceful degradation
   - Verify parser doesn't create invalid AST structures

2. **AST Validation Framework**
   - Implement automated AST structure validation
   - Check for redundant wrappers in CI pipeline
   - Verify semantic correctness of parsed structures

3. **Parser Quality Metrics**
   - Track percentage of statements that parse to structured AST vs. tokens
   - Monitor parser performance and memory usage
   - Set quality gates for new dialect features

## Conclusion

The T-SQL dialect test expectations reveal several categories of parsing issues that impact both correctness and performance. While many basic SQL constructs parse correctly, complex statements and edge cases show significant problems. Addressing the high-priority parser failures should be the immediate focus, followed by systematic cleanup of AST structure inconsistencies.

The redundant wrappers and malformed references suggest that the parser grammar may need refinement to produce cleaner, more consistent AST structures. This analysis provides a roadmap for improving T-SQL dialect support quality and reliability.

---

**Generated**: 2025-01-04  
**Files Analyzed**: 90+ T-SQL dialect test expectation files  
**Analysis Scope**: AST structure patterns, parser behavior, semantic correctness