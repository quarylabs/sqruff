# T-SQL Dialect Test Fixtures Comprehensive Analysis

## Executive Summary

Based on comprehensive analysis of 159 T-SQL dialect test fixtures in `/home/fank/repo/sqruff/crates/lib-dialects/test/fixtures/dialects/tsql/`, the T-SQL parsing implementation shows **significant parsing quality issues** despite recent improvements. While some constructs parse correctly, many critical T-SQL features are being parsed as generic "word" tokens instead of proper SQL AST nodes, indicating fundamental parsing problems.

### Overall Assessment: ⚠️ **NEEDS MAJOR IMPROVEMENTS**

- **Parsing Accuracy**: Moderate to Poor (40-60% proper parsing)
- **Feature Coverage**: Extensive (covers most T-SQL features)
- **Critical Issues**: Multiple structural parsing failures
- **Dialect Maturity**: Early stage with significant gaps

## Test Coverage Analysis

### 1. File Distribution by Category

| Category | Count | Examples |
|----------|-------|----------|
| **DDL Statements** | 45+ | CREATE/ALTER/DROP TABLE, INDEX, VIEW, PROCEDURE |
| **DML Statements** | 35+ | SELECT, INSERT, UPDATE, DELETE, MERGE |
| **T-SQL Procedures/Functions** | 20+ | Stored procedures, functions, parameters |
| **Control Flow** | 15+ | IF/ELSE, WHILE, TRY/CATCH, BEGIN/END |
| **T-SQL Specific Features** | 25+ | Variables, system functions, GOTO, cursors |
| **Advanced Features** | 19+ | Triggers, indexes, security, external tables |

### 2. Feature Coverage Strengths

The test suite provides **excellent coverage** of T-SQL features:

- ✅ Comprehensive stored procedure variations
- ✅ Multiple index creation patterns  
- ✅ Complex SELECT statement constructs
- ✅ T-SQL variables and system functions
- ✅ Advanced features like triggers, cursors, bulk operations
- ✅ Error handling patterns (TRY/CATCH)
- ✅ Security-related constructs (certificates, keys)

## Critical Parsing Issues Identified

### 1. **SEVERE: Control Flow Statements**

**Files Affected**: `try_catch.yml`, `if_else.yml`, `while_statement.yml`

**Issue**: TRY/CATCH blocks and complex IF/ELSE statements parsed entirely as generic "word" tokens.

**Example from `try_catch.yml`**:
```yaml
- statement:
  - word: BEGIN
  - word: TRY
  - word: SELECT
  # ... entire block as "word" tokens
```

**Impact**: ❌ **CRITICAL** - Core T-SQL control flow unusable for linting rules.

### 2. **SEVERE: DDL Index Statements**

**Files Affected**: `add_index.yml`, `alter_index.yml`

**Issue**: Entire CREATE INDEX, DROP INDEX, and UPDATE STATISTICS statements parsed as "word" tokens.

**Example from `add_index.yml`**:
```yaml
- statement:
  - word: CREATE
  - word: NONCLUSTERED
  - word: INDEX
  # ... entire statement as "word" tokens
```

**Impact**: ❌ **CRITICAL** - Index management statements completely unparsable.

### 3. **SEVERE: Stored Procedure Parameters**

**Files Affected**: `function_default_params.yml`, `function_no_return.yml`, `stored_procedure_begin_end.yml`

**Issue**: Procedure parameters not properly wrapped in brackets, causing parameter lists to be parsed as loose tokens.

**Example from `function_default_params.yml`**:
```yaml
- create_procedure_statement:
  - keyword: CREATE
  - keyword: PROCEDURE
  - object_reference: [...]
  - tsql_variable: '@param1'    # ❌ Should be inside bracketed parameter list
  - data_type: [...]
```

**Impact**: ❌ **CRITICAL** - Procedure parameter validation impossible.

### 4. **MAJOR: Trigger Statements**

**Files Affected**: `triggers.yml`

**Issue**: Multiple trigger definitions parsed incorrectly with large portions as generic expressions.

**Example**: Second trigger completely fragmented into expression/column_reference nodes instead of proper trigger syntax.

**Impact**: ❌ **MAJOR** - Trigger analysis severely compromised.

### 5. **MODERATE: IF Statement Conditions**

**Files Affected**: `if_else.yml`, `declare_with_following_statements.yml`

**Issue**: Complex IF conditions often fallback to "word" tokens instead of proper expression parsing.

**Example**: `IF @id IS NULL` parsed as:
```yaml
- tsql_variable: '@id'
- word: IS
- word: 'NULL'
```

**Impact**: ⚠️ **MODERATE** - Some conditional logic analysis affected.

## Parsing Quality by Category

### ✅ **GOOD Parsing Quality**

1. **System Variables** (`system-variables.yml`)
   - T-SQL system variables (@@ROWCOUNT, @@ERROR, etc.) parse correctly
   - Proper `tsql_variable` node assignment

2. **Basic Stored Procedures** (`create_procedure.yml`)
   - Simple CREATE/ALTER PROCEDURE statements parse well
   - BEGIN/END blocks handled properly
   - Complex parameter scenarios with defaults and OUTPUT work

3. **OPEN SYMMETRIC KEY** (`open_symmetric_key.yml`)
   - Security-related statements parse correctly
   - Proper keyword recognition for certificates and keys

4. **Variable Declarations** (`declare_with_following_statements.yml`)
   - DECLARE statements with assignments parse correctly
   - Data types and default values handled properly

### ⚠️ **MIXED Parsing Quality**

1. **SELECT Statements** 
   - Basic SELECT parsing good
   - Complex joins and subqueries sometimes fall back to "word" tokens
   - Window functions need improvement

2. **Procedure Bodies**
   - Simple statements within procedures parse correctly
   - Complex control flow falls back to generic tokens

### ❌ **POOR Parsing Quality**

1. **DDL Index Operations**
   - CREATE/DROP INDEX statements completely unparsed
   - Statistics operations fail

2. **Control Flow**
   - TRY/CATCH blocks fail completely
   - Complex IF/ELSE scenarios problematic

3. **Trigger Definitions**
   - Multiple trigger syntax variations fail

## Specific Technical Issues

### Issue 1: Missing Statement Classifications
Many T-SQL constructs lack proper statement-level classification:
- `CREATE INDEX` should be `create_index_statement`
- `TRY/CATCH` should be `try_catch_statement`
- `UPDATE STATISTICS` should be `update_statistics_statement`

### Issue 2: Parameter List Parsing
Stored procedure and function parameters not properly bracketed, causing:
- Individual parameters parsed as separate statement elements
- Parameter validation rules cannot function
- Default value assignments disconnected from parameters

### Issue 3: Context-Dependent Keywords
T-SQL keywords not being recognized in specific contexts:
- `TRY`/`CATCH` treated as identifiers instead of keywords
- `INDEX` in CREATE INDEX not recognized as keyword
- Context-sensitive keyword resolution failing

### Issue 4: Complex Expression Fallback
Complex expressions falling back to generic "word" tokens:
- Nested EXISTS clauses
- Complex conditional expressions  
- Multi-part object names in certain contexts

## Recommendations for Improvement

### Priority 1: Critical Fixes

1. **Implement TRY/CATCH Statement Support**
   - Add `try_catch_statement` AST node
   - Implement proper BEGIN TRY...END TRY...BEGIN CATCH...END CATCH parsing
   - Target files: `try_catch.yml`

2. **Fix DDL Index Statement Parsing**
   - Add `create_index_statement`, `drop_index_statement` nodes
   - Implement CREATE/DROP INDEX syntax parsing
   - Add UPDATE STATISTICS support
   - Target files: `add_index.yml`, `alter_index.yml`

3. **Fix Stored Procedure Parameter Parsing**
   - Ensure parameters are properly wrapped in brackets
   - Fix parameter list parsing in procedure signatures
   - Target files: `function_default_params.yml`, `function_no_return.yml`

### Priority 2: Major Improvements

4. **Improve Trigger Statement Parsing**
   - Fix CREATE TRIGGER statement parsing
   - Handle AFTER/BEFORE/INSTEAD OF properly
   - Target files: `triggers.yml`

5. **Enhance Complex IF/ELSE Parsing**
   - Improve conditional expression parsing
   - Fix IS NULL/IS NOT NULL recognition
   - Target files: `if_else.yml`

### Priority 3: Quality Enhancements

6. **Improve Context-Dependent Keyword Recognition**
   - Enhance lexer to recognize T-SQL keywords in context
   - Reduce fallback to generic "word" tokens

7. **Add Missing Statement Types**
   - BULK INSERT statements
   - WAITFOR statements
   - GOTO/LABEL statements
   - Complex MERGE statement variations

## Test Quality Assessment

### Strengths
- **Comprehensive coverage** of T-SQL features
- **Good variety** of syntax patterns and edge cases
- **Real-world examples** that reflect actual T-SQL usage
- **Progressive complexity** from simple to advanced constructs

### Recommendations for Test Improvement
- Add more incremental test cases for failing constructs
- Break down complex multi-statement files into focused tests
- Add specific tests for edge cases once parsing is fixed

## Conclusion

The T-SQL dialect test fixtures provide excellent coverage of T-SQL features, but reveal **significant parsing implementation gaps**. While basic constructs like variable declarations and simple procedures parse correctly, critical features like control flow statements, DDL operations, and complex procedure parameters fail to parse properly.

**Immediate Action Required**: The parsing issues identified represent fundamental problems that severely limit the usefulness of T-SQL dialect support. Priority should be given to implementing proper AST nodes for TRY/CATCH, CREATE INDEX, and fixing procedure parameter parsing.

**Estimated Effort**: Addressing the critical issues would require significant dialect grammar work, likely 2-3 weeks of focused development to bring T-SQL parsing to production quality.

**Current State**: The T-SQL dialect is in early development stage with extensive test coverage but substantial parsing gaps that prevent effective SQL analysis and linting.