# T-SQL CASE Expression Parsing Debug Summary

## Issue Description
CASE expressions are unparsable in T-SQL SELECT clauses but work correctly in WHERE clauses.

## Test Results

### SELECT Clause (BROKEN)
```sql
SELECT CASE WHEN Status = 'Active' THEN 'A' END AS StatusCode
```
- **Result**: 2 unparsable sections found
- **Location**: The CASE expression itself becomes unparsable within SelectClauseElementSegment

### WHERE Clause (WORKING)
```sql  
SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1
```
- **Result**: 0 unparsable sections found
- **Location**: CASE expression parses correctly as part of the WHERE expression

## Token Analysis
All CASE-related keywords are being lexed as generic `Word` tokens:
- Token 2: Word 'CASE' (Word)
- Token 4: Word 'WHEN' (Word)  
- Token 12: Word 'THEN' (Word)
- Token 16: Word 'END' (Word)

## Root Cause
The issue is documented in tsql.rs with a commented-out SelectClauseElementSegment override:

```rust
// T-SQL supports alternative alias syntax: AliasName = Expression
// IMPORTANT: This is currently commented out because it's causing CASE expressions
// to be unparsable in SELECT clauses. The issue is that CASE is being lexed as a
// word token and not converted to a keyword token during parsing.
// TODO: Fix this by ensuring keyword matching happens before identifier matching
/*
dialect.replace_grammar(
    "SelectClauseElementSegment",
    ...
);
*/
```

## Grammar Chain
The normal parsing chain should be:
- `SelectClauseElementSegment` → `BaseExpressionElementGrammar` → `ExpressionSegment` → 
- `Expression_A_Grammar` → `Expression_C_Grammar` → `CaseExpressionSegment`

## Current State
1. ANSI dialect properly includes CaseExpressionSegment in Expression_C_Grammar
2. T-SQL inherits this from ANSI
3. CASE is in T-SQL's reserved keywords list
4. But something is preventing proper keyword recognition in SELECT clauses

## Hypothesis
There may be a T-SQL-specific grammar rule or lookahead that's interfering with CASE keyword recognition in SELECT clauses specifically. The fact that it works in WHERE clauses suggests the parser context affects keyword matching.

## Next Steps
1. Investigate if there's a T-SQL-specific override affecting SelectClauseElementSegment
2. Check if there are any lookahead excludes or terminators affecting CASE parsing
3. Verify the keyword matching order in T-SQL dialect