# T-SQL CASE Expression Parsing Issue - Summary and Solution

## Problem Statement
CASE expressions are unparsable in T-SQL SELECT clauses but work correctly in WHERE clauses.

## Root Cause
The T-SQL lexer is producing `Word` tokens for all keywords (including CASE, WHEN, THEN, END) instead of `Keyword` tokens. The parser relies on matching these as keywords through `Ref::keyword("CASE")`, but this matching is failing in SELECT clause contexts.

## Evidence
1. Token analysis shows:
   - `Token: Word 'CASE' (is_keyword: false)`
   - `Token: Word 'SELECT' (is_keyword: false)`
   
2. CASE is properly registered in T-SQL's reserved keywords list

3. The issue affects all keywords, not just CASE

4. CASE expressions work in WHERE clauses, suggesting context-specific parsing issue

## Why It Works in WHERE Clauses
In WHERE clauses, the expression parser has a different context that allows word-to-keyword conversion to work properly. The SELECT clause context appears to have something that prevents this conversion.

## Suspected Cause
The commented-out code in tsql.rs mentions:
```rust
// T-SQL supports alternative alias syntax: AliasName = Expression
// IMPORTANT: This is currently commented out because it's causing CASE expressions
// to be unparsable in SELECT clauses. The issue is that CASE is being lexed as a
// word token and not converted to a keyword token during parsing.
```

This suggests there was an attempt to add T-SQL's alternative alias syntax that interfered with keyword recognition.

## Solution Options

### Option 1: Fix Lexer to Produce Keyword Tokens
Modify the T-SQL lexer to produce `Keyword` tokens for reserved keywords during lexing, similar to how other dialects work.

### Option 2: Fix Parser Context in SELECT Clauses
Investigate why the parser's word-to-keyword conversion works in WHERE clauses but not SELECT clauses, and fix the context-specific issue.

### Option 3: Add Explicit Keyword Matching
Add a grammar rule that explicitly handles Word tokens that match reserved keywords and converts them to keyword tokens before matching.

## Recommended Approach
Option 1 is the cleanest solution - ensure the lexer produces proper Keyword tokens for all reserved keywords. This would align T-SQL with how other dialects work and prevent similar issues in the future.