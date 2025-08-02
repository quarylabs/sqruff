# T-SQL Context-Dependent Lexing Deep Analysis

## Summary
After deep investigation, the context-dependent keyword lexing in T-SQL appears to be an architectural limitation in Sqruff where the lexer state changes after certain constructs, causing keywords to be lexed as generic word tokens instead of being recognized as keywords.

## Evidence

### 1. Function/Procedure Bodies After `AS`
```sql
CREATE PROCEDURE Test AS
BEGIN  -- 'BEGIN' lexed as word token
    PRINT 'test'  -- 'PRINT' lexed as word token
END  -- 'END' lexed as word token
```

### 2. After THROW with Parameters
```sql
THROW 50005, N'error', 1;
BEGIN TRY  -- 'BEGIN', 'TRY' lexed as word tokens
    EXEC spSomeProc  -- 'EXEC' lexed as word token
END TRY  -- 'END', 'TRY' lexed as word tokens
```

## Key Findings

1. **Workarounds Already Exist**: The T-SQL dialect already has partial workarounds that accept word tokens for some statements:
   - PRINT accepts both keyword and word tokens
   - BEGIN/END accepts both keyword and word tokens
   - IF/ELSE accepts both keyword and word tokens
   - Several other statements have similar patterns

2. **But They Don't Work in All Contexts**: Despite these workarounds, procedures still fail to parse because:
   - The parser may give up before reaching the grammar with word support
   - The context is too ambiguous when everything is a word token
   - Some critical keywords don't have word token support

3. **Lexer State Issue**: The root cause appears to be that the lexer enters a different state after certain tokens:
   - After `AS` in CREATE PROCEDURE/FUNCTION
   - After THROW statements with parameters
   - The lexer then treats all alphanumeric sequences as words

## Why This Happens

Sqruff's architecture:
1. **Lexing Phase**: Produces tokens based on patterns
2. **Parsing Phase**: Matches tokens against grammar rules

The issue is that the lexer doesn't maintain context about what SQL construct it's in. When it encounters certain patterns, it changes how it tokenizes subsequent text.

## Attempted Solutions

1. **keyword_or_word pattern** (removed): Created a helper to accept both keyword and word tokens - didn't solve the root issue

2. **Individual word token support** (partially exists): Added StringParser patterns for word tokens - helps in some cases but not all

3. **Missing comprehensive solution**: Would require either:
   - Context-aware lexing (major architectural change)
   - Complete word token support for ALL keywords (massive undertaking)
   - Post-lexing token conversion (architectural change)

## Recommendation

Accept this as a known limitation. The 2 unparsable files represent edge cases that would require disproportionate effort to fix. Users can work around this by:
1. Using square bracket escaping in T-SQL
2. Restructuring their SQL to avoid the problematic patterns
3. Using GO statements to reset lexer state