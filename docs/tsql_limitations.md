# T-SQL Dialect Limitations

## Overview

The T-SQL dialect in Sqruff successfully parses 98.74% of test cases (157 out of 159 files). However, there are known limitations due to T-SQL's context-dependent keyword lexing behavior.

## Known Limitations

### 1. Keywords Lexed as Words in Procedure Bodies

In certain contexts, T-SQL keywords are lexed as word tokens instead of keyword tokens. This primarily occurs:

- Inside stored procedure/function bodies after the `AS` keyword
- After `THROW` statements with parameters and semicolons

**Example - Unparsable Code:**
```sql
CREATE PROCEDURE findjobs @nm sysname = NULL
AS
IF @nm IS NULL      -- 'IF', 'IS', 'NULL' are lexed as words
    BEGIN           -- 'BEGIN' is lexed as a word
        PRINT 'You must give a user name'  -- 'PRINT' is lexed as a word
        RETURN      -- 'RETURN' is lexed as a word
    END             -- 'END' is lexed as a word
```

### 2. Context-Dependent Lexing After THROW

Keywords following a `THROW` statement with parameters can be lexed as words:

**Example - Partially Unparsable:**
```sql
THROW 50005, N'an error occurred', 1;

BEGIN TRY           -- May be lexed as words in certain contexts
    EXEC spSomeProc
END TRY
BEGIN CATCH
END CATCH
```

## Workarounds

### Option 1: Use Square Bracket Escaping

T-SQL provides square bracket escaping for identifiers that conflict with keywords:

```sql
CREATE PROCEDURE Test AS
[BEGIN]  -- Explicitly escaped
    [IF] @x = 1
    [PRINT] 'test'
[END]
```

### Option 2: Restructure Code

Avoid patterns that trigger the lexing issues:
- Place `GO` statements before problematic constructs
- Use alternative syntax where available

## Technical Background

This limitation stems from Sqruff's architecture:
1. The lexer produces generic word tokens for all alphanumeric sequences
2. Keywords are identified during parsing using text matching
3. In certain T-SQL contexts, the lexer state changes and keywords remain as word tokens
4. This is a fundamental architectural constraint that cannot be easily resolved

## Impact

- **Affected Files**: 2 out of 159 test files (1.26%)
- **Common Scenarios**: Complex stored procedures, functions without return types
- **Severity**: Low - most T-SQL code parses correctly

## Comparison with SQLFluff

SQLFluff has similar issues with T-SQL parsing, including:
- Crashes with keyword-named columns (Issue #5828)
- Stored procedures unparsable (Issue #5239)
- T-SQL FETCH cursor issues (Issue #6011)

Sqruff's 98.74% success rate represents strong T-SQL support despite these limitations.