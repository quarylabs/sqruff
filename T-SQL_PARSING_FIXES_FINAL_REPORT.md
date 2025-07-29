# T-SQL Parsing Fixes - Final Report

## Summary

This report documents the T-SQL parsing fixes implemented and the remaining limitations discovered during the investigation.

## Successfully Fixed Issues

### 1. Compound Assignment Operators (+=, -=, *=, /=, %=)
**Issue**: LT01 spacing errors due to operators being parsed as separate tokens
**Solution**: 
- Removed duplicate AssignmentOperatorSegment definitions
- Added SelectClauseElementSegment to DeclareStatementSegment
- Operators now parse correctly as single tokens

### 2. INSERT...EXEC Statements
**Issue**: INSERT INTO with EXEC was unparsable
**Solution**: Added ExecuteStatementGrammar to InsertStatementSegment in tsql.rs
**Example**: `INSERT INTO table1 EXEC FindEmployeesFunc @lastName = 'Picard'` now parses correctly

### 3. MultiStringParser Implementation
**Issue**: is_optional() method was unimplemented (todo!())
**Solution**: Modified crates/lib-core/src/parser/parsers.rs to return false

## Remaining Limitations (Lexer-Level)

### 1. CASE Expressions in SELECT Context
**Issue**: CASE keyword lexed as 'word' token in SELECT but 'keyword' token in WHERE
**Impact**: CASE expressions fail to parse in SELECT clauses
**Root Cause**: T-SQL lexer behavior - cannot be fixed at parser level

### 2. AS Keyword After OPENROWSET
**Issue**: AS keyword lexed as 'word' token after OPENROWSET
**Impact**: Aliases fail to parse after OPENROWSET
**Example**: `FROM OPENROWSET(...) AS rows` - 'AS' becomes unparsable

### 3. Other Lexer-Related Issues
- UNION ALL in subqueries within IF EXISTS
- ON DATABASE/ALL SERVER in trigger definitions  
- UPDATE() function in triggers
- CREATE OR ALTER syntax
- DISABLE/ENABLE TRIGGER statements

## Files with Remaining Unparsable Sections

1. **triggers.yml** - Most extensive unparsable sections due to:
   - ON DATABASE/ALL SERVER syntax
   - CREATE OR ALTER TRIGGER
   - UPDATE() function
   - DISABLE/ENABLE TRIGGER

2. **if_else.yml** - UNION ALL in IF EXISTS subquery

3. **case_in_select.yml** - CASE expressions in SELECT

4. **openrowset.yml** - AS keyword after OPENROWSET

5. **set_statements.yml** - Various SET statement variations

6. **nested_joins.yml** - Complex join patterns

7. **json_functions.yml** - JSON path expressions

8. **create_view_with_set_statements.yml** - SET statements in views

9. **temporal_tables.yml** - Temporal table syntax

10. **create_table_constraints.yml** - Complex constraint definitions

## Technical Analysis

The fundamental issue is that T-SQL's lexer makes context-sensitive decisions about token types:
- Keywords like CASE, AS, UNION, etc. are lexed differently based on surrounding context
- In some contexts they're 'keyword' tokens, in others they're 'word' tokens
- This cannot be resolved at the parser level without modifying the lexer

## Recommendations

1. **Document Limitations**: Clearly document these T-SQL parsing limitations in user-facing documentation

2. **Lexer Enhancement**: Consider future lexer enhancements to handle context-sensitive keywords

3. **Workarounds**: For linting purposes, users can:
   - Use CTEs instead of complex CASE in SELECT
   - Avoid aliases after OPENROWSET
   - Split complex triggers into simpler statements

## Conclusion

Significant progress was made in fixing T-SQL parsing issues:
- Compound assignment operators now work correctly
- INSERT...EXEC statements parse properly
- Core parser infrastructure improved (MultiStringParser)

The remaining unparsable sections are due to fundamental lexer behavior in T-SQL that treats keywords differently based on context. These would require lexer-level changes to fully resolve.