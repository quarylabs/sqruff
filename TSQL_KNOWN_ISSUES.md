# T-SQL Dialect Known Issues

This document tracks known issues with the T-SQL dialect implementation.

## Parser Issues

1. **CREATE TABLE statements not fully supported**
   - Temporary tables (`#TempTable`) 
   - `CREATE TABLE ... AS` syntax
   - IDENTITY columns
   - PRIMARY KEY constraints
   - Test case `issue_1639` in AL07.yml disabled

2. **CREATE FUNCTION not supported**
   - T-SQL function definitions cannot be parsed

3. **CREATE PROCEDURE not supported**
   - Stored procedure definitions cannot be parsed

4. **CREATE TYPE not supported**
   - User-defined table types cannot be parsed

## Rule Issues

1. **CP01 - Function name capitalization**
   - Test `test_fail_select_lower_keyword_functions` is failing
   - Function names like `CAST` and `COALESCE` are not being properly capitalized
   - Even though these are defined as keywords in tsql_keywords.rs

2. **CV01 - Multi-line comparison operators**
   - Tests `test_fail_c_style_not_equal_to_tsql` and `test_fail_ansi_not_equal_to_tsql` are failing
   - Cannot properly convert multi-line `<>` operators to `!=` when split across lines
   - Issue occurs when comments are placed between operator parts

3. **CV07 - Outer parentheses removal**
   - Tests `test_fail_outer_brackets_tsql` and `test_fail_outer_brackets_inner_subquery_tsql` are failing
   - Rule not properly removing unnecessary outer parentheses from T-SQL SELECT statements

## Related Issues
- #1674 - T-SQL: table hints like WITH (NOLOCK) are parsed as aliases
- #1675 - T-SQL: Implicit column aliases without AS keyword not supported