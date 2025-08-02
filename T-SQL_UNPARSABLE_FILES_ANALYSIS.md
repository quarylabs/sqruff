# T-SQL Unparsable Files Analysis

## Summary
After removing the `keyword_or_word` pattern, we have 4 unparsable files in T-SQL, representing 2 distinct issues:

1. **Context-dependent keyword lexing** (2 files)
2. **Missing join hint support** (2 files)

## Issue 1: Context-Dependent Keyword Lexing

### Affected Files
- `function_no_return.yml`
- `try_catch.yml` (partially)

### Root Cause
Keywords are lexed as word tokens in specific contexts:

1. **Inside procedure/function bodies after `AS`**
   ```sql
   CREATE PROCEDURE findjobs @nm sysname = NULL
   AS
   IF @nm IS NULL      -- 'IF', 'IS', 'NULL' lexed as words
       BEGIN           -- 'BEGIN' lexed as word
           PRINT 'You must give a user name'  -- 'PRINT' lexed as word
           RETURN      -- 'RETURN' lexed as word
       END             -- 'END' lexed as word
   ```

2. **After `THROW` statement with parameters**
   ```sql
   THROW 50005, N'an error occurred', 1;
   
   BEGIN TRY           -- 'BEGIN', 'TRY' lexed as words
       EXEC spSomeProc -- 'EXEC' lexed as word
   END TRY             -- 'END', 'TRY' lexed as words
   ```

### Why It Happens
This is a fundamental limitation of Sqruff's architecture where the lexer state changes in certain contexts, causing keywords to remain as generic word tokens instead of being identified as keywords.

## Issue 2: Missing Join Hint Support

### Affected Files
- `join_hints.yml`
- `nested_joins.yml`

### Root Cause
T-SQL supports algorithm hints between the join type and the JOIN keyword:
- `INNER HASH JOIN`
- `LEFT LOOP JOIN`
- `FULL OUTER MERGE JOIN`

### Examples
```sql
-- HASH JOIN hint
SELECT table1.col
FROM table1
INNER HASH JOIN table2
    ON table1.col = table2.col;

-- Nested joins with hints
FROM table1 t1
LEFT OUTER HASH JOIN table2 t2
    INNER MERGE JOIN table3 t3
        ON t2.id = t3.t2_id
    ON t1.id = t2.t1_id
```

### Why It Fails
The current T-SQL grammar doesn't recognize join hints (`HASH`, `LOOP`, `MERGE`) between the join type keywords and the `JOIN` keyword. The parser expects `JOIN` immediately after `INNER`, `LEFT OUTER`, etc.

## Statistics
- Total T-SQL test files: 159
- Unparsable files: 4
- Success rate: 97.48%

### Breakdown by Issue
- Context-dependent lexing: 2 files (1.26%)
- Missing join hints: 2 files (1.26%)

## Potential Solutions

### For Context-Dependent Lexing
1. **Workaround**: Use T-SQL's square bracket escaping
   ```sql
   CREATE PROCEDURE Test AS
   [BEGIN]
       [IF] @x = 1
       [PRINT] 'test'
   [END]
   ```

2. **Architectural Change**: Would require significant changes to Sqruff's lexer-parser architecture

### For Join Hints
1. **Grammar Enhancement**: Add support for join hints in the join clause grammar
2. **Implementation**: Modify `JoinClauseSegment` to accept optional hint keywords between join type and JOIN keyword

## Recommendation
1. Document these limitations clearly (already done in `docs/tsql_limitations.md`)
2. Consider implementing join hint support as it's a straightforward grammar enhancement
3. Accept the context-dependent lexing limitation as an architectural constraint