# Disabled T-SQL Tests Summary

This document lists all the T-SQL tests that are currently disabled in the sqruff test suite, as mentioned in issue #1685.

## AL07.yml
- **Test:** `issue_1639`
  - **Reason:** T-SQL CREATE TABLE syntax not fully supported yet
  - **Description:** Test for CREATE TABLE with AS clause

## LT02-indent.yml
- **Test:** `test_pass_tsql_else_if`
  - **Reason:** T-SQL IF/ELSE IF/ELSE indentation not working correctly
  
- **Test:** `test_fail_tsql_else_if`
  - **Reason:** T-SQL IF/ELSE IF/ELSE indentation not working correctly
  
- **Test:** `test_fail_tsql_else_if_successive`
  - **Reason:** T-SQL IF/ELSE IF/ELSE indentation not working correctly
  
- **Test:** `test_tsql_function`
  - **Reason:** T-SQL CREATE FUNCTION syntax not fully supported
  
- **Test:** `test_pass_tsql_declare_indent`
  - **Reason:** T-SQL DECLARE statement indentation not working correctly
  
- **Test:** `test_pass_tsql_set_indent`
  - **Reason:** T-SQL SET statement indentation not working correctly
  
- **Test:** `test_pass_tsql_set_indent_multiple_params`
  - **Reason:** T-SQL SET statement indentation not working correctly
  
- **Test:** `test_pass_tsql_if_indent`
  - **Reason:** T-SQL IF statement indentation not working correctly
  
- **Test:** `test_pass_tsql_index_indent`
  - **Reason:** T-SQL CREATE INDEX indentation not working correctly
  
- **Test:** `test_pass_tsql_statistics_indent`
  - **Reason:** T-SQL CREATE STATISTICS indentation not working correctly
  
- **Test:** `test_pass_indented_procedure_parameters`
  - **Reason:** T-SQL CREATE PROCEDURE syntax not fully supported
  
- **Test:** `test_fail_unindented_procedure_parameters`
  - **Reason:** T-SQL CREATE PROCEDURE syntax not fully supported
  
- **Test:** `test_tsql_bubble_up_newline_after_fix`
  - **Reason:** T-SQL CREATE PROCEDURE syntax not fully supported
  
- **Test:** `test_tsql_nested_join`
  - **Reason:** T-SQL nested JOIN indentation not working correctly

## LT01-excessive.yml
- **Test:** `test_identifier_fix`
  - **Reason:** T-SQL multi-line object reference joining not working correctly
  
- **Test:** `test_comparison_operator_fix`
  - **Reason:** T-SQL split comparison operator joining not working correctly
  
- **Test:** `test_comparison_operator_pass`
  - **Reason:** T-SQL comparison operator >= incorrectly parsed as two tokens
  
- **Test:** `test_fix_tsql_spaced_chars`
  - **Reason:** T-SQL split comparison operator joining not working correctly

## LT01-literals.yml
- **Test:** `test_pass_tsql_unicode_single_quote`
  - **Reason:** T-SQL Unicode string literal N'string' incorrectly parsed as two tokens
  
- **Test:** `test_fail_tsql_unicode_single_quote`
  - **Reason:** T-SQL Unicode string literal N'string' incorrectly parsed as two tokens

## LT01-operators.yml
- **Test:** (unnamed test for compound assignment operators)
  - **Reason:** T-SQL compound assignment operator += incorrectly parsed as two tokens

## CP01.yml
- **Test:** (test for issue #3520)
  - **Reason:** T-SQL function name capitalization not working correctly

## CV01.yml
- **Test:** `test_fail_c_style_not_equal_to_tsql`
  - **Reason:** T-SQL multi-line comparison operator conversion not working correctly
  
- **Test:** `test_fail_ansi_not_equal_to_tsql`
  - **Reason:** T-SQL multi-line comparison operator conversion not working correctly

## CV07.yml
- **Test:** `test_fail_outer_brackets_tsql`
  - **Reason:** T-SQL outer parentheses removal not working correctly
  
- **Test:** `test_fail_outer_brackets_inner_subquery_tsql`
  - **Reason:** T-SQL outer parentheses removal not working correctly

## RF01.yml
- **Test:** `test_nested_join_clause_does_not_flag`
  - **Reason:** T-SQL nested JOIN syntax causes panic in parser
  
- **Test:** `test_nested_join_clauses_do_not_flag`
  - **Reason:** T-SQL nested JOIN syntax causes panic in parser
  
- **Test:** `test_parenthesized_join_clauses_do_not_flag`
  - **Reason:** T-SQL parenthesized JOIN syntax causes panic in parser

## RF02.yml
- **Test:** `test_pass_qualified_references_multi_table_statements_tsql`
  - **Reason:** T-SQL JOIN syntax causes panic in parser
  
- **Test:** `test_fail_unqualified_references_multi_table_statements_tsql`
  - **Reason:** T-SQL JOIN syntax causes panic in parser
  
- **Test:** (test for issue #2169)
  - **Reason:** T-SQL JOIN syntax causes panic in parser
  
- **Test:** `test_pass_column_and_alias_same_name_1_tsql`
  - **Reason:** T-SQL JOIN syntax causes panic in parser
  
- **Test:** (test for alias detection)
  - **Reason:** T-SQL JOIN syntax causes panic in parser

## ST05.yml
- **Test:** `issue_3623_internal_error_multiple_templated_files`
  - **Reason:** T-SQL CTE syntax handling not working correctly

## Summary of Issues
The disabled T-SQL tests can be categorized into the following main issues:

1. **Parser Issues:**
   - CREATE TABLE with AS clause not fully supported
   - CREATE FUNCTION syntax not fully supported
   - CREATE PROCEDURE syntax not fully supported
   - CREATE INDEX indentation issues
   - CREATE STATISTICS indentation issues
   - Unicode string literals (N'string') parsed incorrectly
   - Compound assignment operators (+=, -=, etc.) parsed incorrectly
   - Comparison operators (>=, <=) sometimes parsed as two tokens
   - Multi-line object references not joined correctly
   - CTE syntax handling issues

2. **Indentation Issues:**
   - IF/ELSE IF/ELSE statement indentation
   - DECLARE statement indentation
   - SET statement indentation
   - Nested JOIN indentation
   - Procedure parameter indentation

3. **JOIN Handling Issues:**
   - Nested JOIN syntax causes parser panic
   - Parenthesized JOIN syntax causes panic
   - JOIN syntax in general causing issues in RF01/RF02 rules

4. **Rule-Specific Issues:**
   - Function name capitalization (CP01)
   - Multi-line comparison operator conversion (CV01)
   - Outer parentheses removal (CV07)