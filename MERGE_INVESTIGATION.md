# MERGE JOIN Investigation

## Problem Statement

The MERGE keyword in T-SQL creates a parser conflict:
- When used as a JOIN hint (e.g., `INNER MERGE JOIN`, `FULL OUTER MERGE JOIN`), MERGE should be recognized as part of the join type
- When used as a statement (e.g., `MERGE table1 USING table2...`), MERGE should be recognized as starting a MERGE statement
- Currently, the parser always tries to parse MERGE as a statement first, causing JOIN patterns with MERGE to fail

## Current Symptoms

### What Works
- `INNER HASH JOIN` ✓
- `LEFT LOOP JOIN` ✓  
- `FULL HASH JOIN` ✓
- `MERGE` statements (when not in JOIN context) ✓

### What Fails
- `MERGE JOIN` ✗
- `INNER MERGE JOIN` ✗
- `FULL OUTER MERGE JOIN` ✗
- Any JOIN pattern with MERGE hint ✗

### Error Pattern
```
L:   1 | P:  31 | ???? | Unparsable section
```
The unparsable section starts right at the MERGE keyword in JOIN context.

## Investigation Plan

### Phase 1: Understanding the Parser Architecture
- [ ] Map out how the parser handles keyword precedence
- [ ] Understand the relationship between StatementSegment and JoinClauseSegment parsing
- [ ] Identify where MERGE is first recognized and how that decision is made
- [ ] Document the parser flow from SQL text → tokens → AST

### Phase 2: Analyzing Current Implementation
- [ ] Review how ANSI handles similar keyword conflicts
- [ ] Check if other dialects have similar issues with dual-purpose keywords
- [ ] Examine the current JoinTypeKeywordsGrammar implementation
- [ ] Trace through parser execution for "FROM table1 MERGE JOIN table2"

### Phase 3: Exploring Solutions
- [ ] Test if context-aware parsing is possible (e.g., different rules inside FROM clause)
- [ ] Investigate if we can use lookahead to disambiguate MERGE usage
- [ ] Consider if we need a custom lexer rule for T-SQL
- [ ] Explore parser precedence/priority mechanisms

### Phase 4: Implementation Attempts
- [ ] Document each implementation attempt with:
  - What was tried
  - Why it was expected to work
  - What actually happened
  - Lessons learned

## Research Log

### Entry 1: Initial Understanding
**Date**: 2025-07-28
**Finding**: The issue occurs because when the parser encounters "MERGE", it attempts to match it against MergeStatementSegment at the top level before considering it as part of JoinTypeKeywordsGrammar.

**Evidence**:
- Disabling MergeStatementSegment in StatementSegment list doesn't fix the issue
- Using StringParser instead of Ref::keyword for MERGE doesn't help
- The problem persists even with explicit MERGE patterns in JoinTypeKeywordsGrammar

### Entry 2: Parser Flow Investigation
**Date**: 2025-07-28
**Goal**: Understand exactly how the parser processes "SELECT * FROM table1 MERGE JOIN table2"
**Method**: Code analysis of parser internals

**Findings**:
1. **StringParser vs Ref::keyword**:
   - `StringParser::new("MERGE", SyntaxKind::Keyword)` matches raw segment text directly
   - `Ref::keyword("MERGE")` looks up "MERGE" in the dialect's grammar definitions
   - Both use case-insensitive matching
   
2. **Parser Flow**:
   - When `Ref::keyword("MERGE")` is used, it calls `parse_context.dialect().r#ref("MERGE")`
   - This looks up a grammar element named "MERGE" in the dialect
   - The dialect likely has a pre-defined "MERGE" keyword that maps to MergeStatementSegment

3. **Hypothesis**:
   - The issue might be that "MERGE" is pre-defined at the dialect level as a keyword
   - This keyword definition might be triggering MERGE statement parsing
   - Using StringParser doesn't help because the conflict happens at a higher level

### Entry 3: Keyword Definition Investigation
**Date**: 2025-07-28
**Goal**: Find where MERGE is defined as a keyword and how it's linked to MergeStatementSegment

**Findings**:
1. **Dialect Library System**:
   - Dialect has a `library: AHashMap<Cow<'static, str>, DialectElementType>` that stores grammar elements
   - When `Ref::keyword("MERGE")` is called, it looks up "MERGE" in this library
   - The `add` method adds elements to the library: `dialect.add([(name, element)])`

2. **Keyword Registration**:
   - Keywords seem to be automatically added to the library somewhere
   - Need to find where/how "MERGE" gets registered as a keyword element

### Entry 4: Keyword Auto-Registration Investigation
**Date**: 2025-07-28
**Goal**: Find how keywords are automatically registered in the dialect

**Critical Discovery**:
1. **Automatic Keyword Registration**:
   - `Dialect::expand()` automatically creates `StringParser` entries for all keywords
   - Keywords are defined in `ANSI_UNRESERVED_KEYWORDS` and `ANSI_RESERVED_KEYWORDS`
   - MERGE is in `ANSI_UNRESERVED_KEYWORDS` (line 439 in ansi_keywords.rs)
   - During expand: `StringParser::new("MERGE", SyntaxKind::Keyword)` is added to library

2. **Why StringParser Doesn't Help**:
   - Even when we use `StringParser::new("MERGE", SyntaxKind::Keyword)` in JoinTypeKeywordsGrammar
   - Both match the same way - they're functionally identical
   - The issue is NOT about how MERGE is matched, but WHEN/WHERE it's matched

### Entry 5: Parser Context Investigation
**Date**: 2025-07-28  
**Goal**: Understand why MERGE is being parsed as a statement even in JOIN context

**Parser Flow Analysis**:
1. **FROM Clause Structure**:
   ```
   FromClauseSegment = FROM + Delimited(FromExpressionSegment)
   FromExpressionSegment = FromExpressionElementSegment + AnyNumberOf(JoinClauseSegment)
   ```

2. **Expected Flow for "FROM table1 MERGE JOIN table2"**:
   - FROM → triggers FromClauseSegment
   - table1 → parsed as FromExpressionElementSegment ✓
   - MERGE JOIN → should be parsed as JoinClauseSegment ✗

3. **The Problem**:
   - After parsing table1, parser should look for JoinClauseSegment
   - JoinClauseSegment includes JoinTypeKeywordsGrammar which has MERGE patterns
   - But somehow MERGE is being interpreted differently

### Entry 6: Debugging Strategy
**Date**: 2025-07-28
**Goal**: Add debug output to trace exact parser decisions

**Test Results**:
- Confirmed: "SELECT * FROM t1 MERGE JOIN t2" fails at position 18 (start of MERGE)
- The parser successfully parses up to "t1" then fails on "MERGE"
- Error occurs exactly where MERGE keyword starts in JOIN context

### Entry 7: New Approach - Explicit MERGE JOIN Pattern
**Date**: 2025-07-28
**Goal**: Create an explicit pattern for "MERGE JOIN" sequence
**Rationale**: 
- Instead of relying on JoinTypeKeywordsGrammar + JoinKeywordsGrammar
- Create a specific pattern that matches "MERGE JOIN" as a unit
- This might take precedence over MERGE being interpreted as a statement

**Implementation Ideas**:
1. Override JoinClauseSegment with explicit "MERGE JOIN" sequence
2. Add "MERGE JOIN" as a special case before general JOIN patterns
3. Use lookahead to detect "MERGE" followed by "JOIN"

### Entry 8: Explicit JoinClauseSegment Override Failed
**Date**: 2025-07-28
**What**: Created explicit MERGE JOIN patterns in JoinClauseSegment
**Result**: Still unparsable at position 18
**Learning**: The issue happens before JoinClauseSegment is even tried

### Entry 9: Root Cause Hypothesis
**Date**: 2025-07-28
**Theory**: The parser is making decisions at the FromExpressionSegment level
- After parsing "table1", it sees "MERGE"
- Before trying JoinClauseSegment, something else is matching/failing
- Possible culprits:
  1. Delimited in FromClauseSegment might be terminating early
  2. FromExpressionSegment might have terminators that stop at MERGE
  3. Parser might be trying to start a new statement

## Attempted Solutions

### Attempt 1: Override JoinTypeKeywordsGrammar
**What**: Added MERGE patterns to JoinTypeKeywordsGrammar with most specific patterns first
**Result**: Failed - MERGE still parsed as statement
**Learning**: Grammar precedence alone doesn't solve the issue

### Attempt 2: Custom JoinClauseSegment
**What**: Created T-SQL specific JoinClauseSegment with explicit MERGE handling
**Result**: Failed - Type errors and complexity issues
**Learning**: Need simpler approach, full override too complex

### Attempt 3: StringParser for MERGE
**What**: Used StringParser instead of Ref::keyword for MERGE in JOIN context
**Result**: Failed - No difference in parsing behavior
**Learning**: The issue is not about how MERGE is defined but when it's matched

### Attempt 4: Disable MergeStatementSegment
**What**: Temporarily commented out MergeStatementSegment from StatementSegment
**Result**: Failed - MERGE JOIN still unparsable
**Learning**: The conflict might be happening at a different level than expected

## Next Steps

1. **Deep Dive into Parser**: Need to understand where exactly MERGE is being matched
2. **Debug Tracing**: Add logging to see parser decision flow
3. **Study ANSI Precedence**: How does ANSI handle keyword conflicts?
4. **Context-Aware Parsing**: Can we make FROM clause context influence keyword interpretation?

## Parser Flow Analysis

### Entry 10: Understanding longest_match
**Date**: 2025-07-28
**Finding**: The parser uses `longest_match` to choose between alternatives in `one_of`
- It prunes options based on the first non-whitespace token
- It tries all remaining options and picks the one with the best (longest) match
- Key insight: MergeStatementSegment shouldn't even be an option inside FROM clause

### Entry 11: The Real Issue - Parser Context
**Date**: 2025-07-28
**Theory**: The problem is NOT at the JoinClauseSegment level but higher up
- When parser finishes "FROM t1", it's at the end of FromExpressionElementSegment
- It should then try to match JoinClauseSegment as part of FromExpressionSegment
- But something is causing the parser to exit FromExpressionSegment prematurely

**Hypothesis**: The issue might be in how Delimited works in FromClauseSegment
- `Delimited::new(vec_of_erased![Ref::new("FromExpressionSegment")])`
- Delimited might be seeing MERGE and deciding it's not part of the delimited list

## RESOLUTION

### Entry 12: Issue Already Fixed!
**Date**: 2025-07-28
**Finding**: The MERGE JOIN issue was already fixed in commit cd42ea10

**Test Results**:
All MERGE JOIN patterns now parse successfully:
- ✓ `SELECT * FROM t1 MERGE JOIN t2 ON t1.id = t2.id;`
- ✓ `SELECT * FROM t1 INNER MERGE JOIN t2 ON t1.id = t2.id;`
- ✓ `SELECT * FROM t1 LEFT OUTER MERGE JOIN t2 ON t1.id = t2.id;`
- ✓ `SELECT * FROM t1 RIGHT OUTER MERGE JOIN t2 ON t1.id = t2.id;`
- ✓ `SELECT * FROM t1 FULL OUTER MERGE JOIN t2 ON t1.id = t2.id;`
- ✓ Multiple MERGE JOINs in one query
- ✓ MERGE statements continue to work correctly

**How it was fixed**: 
The fix was implemented by overriding JoinClauseSegment in the T-SQL dialect
(see commit cd42ea10)

Key changes:
1. Created `TsqlJoinTypeKeywordsGrammar` that combines:
   - Join type (INNER, FULL/LEFT/RIGHT [OUTER]) - optional
   - Join hint (LOOP/HASH/MERGE) - optional
   
2. This approach treats MERGE as a join hint when it appears after a join type,
   avoiding the conflict with MergeStatementSegment

3. The pattern allows for:
   - `MERGE JOIN` (just the hint + JOIN keyword)
   - `INNER MERGE JOIN` (type + hint + JOIN)
   - `FULL OUTER MERGE JOIN` (full type + hint + JOIN)

This elegant solution preserves MERGE statement functionality while enabling
MERGE as a valid join hint in T-SQL.

## Questions to Answer

1. Where in the parser does MERGE first get recognized?
2. Is there a way to influence parser precedence based on context?
3. How do other SQL parsers handle this ambiguity?
4. Can we use lookahead (e.g., MERGE followed by JOIN vs MERGE followed by table name)?
5. Is this a lexer-level or parser-level issue?

## Success Criteria

The following SQL should parse without errors:
```sql
-- Simple MERGE JOIN
SELECT * FROM table1 MERGE JOIN table2 ON table1.id = table2.id;

-- With join type
SELECT * FROM table1 INNER MERGE JOIN table2 ON table1.id = table2.id;
SELECT * FROM table1 FULL OUTER MERGE JOIN table2 ON table1.id = table2.id;

-- MERGE statement should still work
MERGE table1 USING table2 ON table1.id = table2.id WHEN MATCHED THEN UPDATE SET col = 1;
```

## Test Status

### Existing Test File
The file `crates/lib-dialects/test/fixtures/dialects/tsql/join_hints.yml` shows that:
- ✓ `INNER HASH JOIN` parses correctly 
- ✗ `FULL OUTER MERGE JOIN` still has unparsable sections (lines 65-80)
- ✗ `LEFT LOOP JOIN` appears in unparsable section (lines 87-99)

**UPDATE**: After checking commit cd42ea10, the join hints implementation was added but the
test expectations were NOT updated. The YAML file still shows unparsable sections for
MERGE JOIN patterns that should now parse correctly.

### Full List of Files with Unparsable Sections

Running `./.hacking/scripts/check_for_unparsable.sh` reveals 17 T-SQL files still have
unparsable sections:

1. `case_in_select.yml`
2. `create_table_constraints.yml`
3. `create_table_with_sequence_bracketed.yml`
4. `create_view.yml`
5. `create_view_with_set_statements.yml`
6. `join_hints.yml` - Contains MERGE JOIN patterns that should now parse
7. `json_functions.yml`
8. `merge.yml`
9. `nested_joins.yml`
10. `openrowset.yml`
11. `select.yml`
12. `select_date_functions.yml`
13. `select_natural_join.yml`
14. `table_object_references.yml`
15. `temporal_tables.yml`
16. `triggers.yml`
17. `update.yml`

These test fixtures need to be regenerated to reflect the current parser capabilities.

## Current Status

- **Fixed**: MERGE JOIN now parses correctly after restructuring JoinClauseSegment
- When linting join_hints.sql directly, no parsing errors occur
- **Issue**: Test fixtures still show unparsable sections - regeneration not working properly
- **Discovery**: The issue may be in how the parser recovers from failures. When "FULL OUTER MERGE JOIN" fails to parse, it might be trying to parse "MERGE JOIN" as a standalone MERGE statement instead of as part of the JOIN clause.

### Entry 13: Regeneration Issue Investigation
**Date**: 2025-07-28
**Problem**: Running `env UPDATE_EXPECT=1 cargo test` doesn't properly regenerate join_hints.yml
**Findings**:
1. When running lint directly on join_hints.sql, it parses successfully
2. But the test fixture regeneration still shows MERGE JOIN as unparsable
3. This suggests there might be an issue with the test harness itself or how it's building the parser

**Next Steps**:
1. Check if there's a difference between how the CLI parser and test parser are initialized
2. Verify the T-SQL dialect is properly loaded in the test environment
3. Look for any caching issues that might prevent the updated grammar from being used

### Entry 14: Parser Discrepancy Found
**Date**: 2025-07-28
**Critical Finding**: The lint CLI correctly parses MERGE JOIN patterns, but the test harness doesn't

**Evidence**:
1. Running `cargo run -p sqruff -- lint` on join_hints.sql → SUCCESS (no parsing errors)
2. Running dialect tests with UPDATE_EXPECT=1 → FAILURE (shows MERGE JOIN as unparsable)
3. Even simple "SELECT * FROM t1 MERGE JOIN t2" fails in test harness

**Hypothesis**:
- The JoinClauseSegment override might not be applied correctly in the test environment
- The issue appears to be at the FromExpressionSegment level - it parses "table1" then fails on "FULL OUTER"
- The test parser might be using a different initialization path than the CLI

**Investigation needed**:
1. Trace how dialect.expand() interacts with replace_grammar() calls
2. Check if the test harness builds the dialect differently
3. Verify the JoinClauseSegment pattern is complete and handles all cases