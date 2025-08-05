# T-SQL ELSE IF Indentation Issue Research

## Problem Statement
The test `test_pass_tsql_else_if` is failing with LT02 violations. The LT02 rule incorrectly expects ELSE and ELSE IF keywords to be indented, when they should be at the same indentation level as the IF keyword.

### Test Case
```sql
IF (1 > 1)
    PRINT 'A';
ELSE IF (2 > 2)
    PRINT 'B';
ELSE IF (3 > 3)
    PRINT 'C';
ELSE
    PRINT 'D';
```

### Expected Behavior
- IF, ELSE IF, and ELSE keywords should be at the same indentation level (0 spaces)
- PRINT statements should be indented (4 spaces)

### Actual Behavior
LT02 reports "Expected indent of 4 spaces" for lines 3, 5, and 7 (the ELSE keywords)

## Investigation Timeline

### 1. Initial Parser Structure Analysis
- Located T-SQL IF statement parser in `/home/fank/repo/sqruff/crates/lib-dialects/src/tsql.rs`
- Found it was missing `MetaSegment::indent()` and `MetaSegment::dedent()` markers

### 2. First Attempt - Basic Indent/Dedent
- Added basic indent/dedent markers around IF body and ELSE body
- Result: Test still failed with same LT02 violations

### 3. Second Attempt - IfStatementsSegment Pattern
- Created `IfStatementsSegment` using `AnyNumberOf` pattern (similar to BigQuery)
- Restructured IF statement with proper indent/dedent positioning
- Result: PRINT statements passed, but ELSE keywords still expected to be indented

### 4. Current Parser Structure (Lines 1790-1851 in tsql.rs)
```rust
// IF statement - includes ELSE handling with proper dedent before ELSE
dialect.add([(
    "IfStatementSegment".into(),
    NodeMatcher::new(SyntaxKind::IfStatement, |_| {
        Sequence::new(vec_of_erased![
            // IF keyword and condition
            one_of(vec_of_erased![/* IF keyword variants */]),
            one_of(vec_of_erased![/* Expression variants */]),
            // Indent for IF body
            MetaSegment::indent(),
            Ref::new("IfStatementsSegment"),
            // Dedent before ELSE - this is critical
            MetaSegment::dedent(),
            // Optional ELSE IF/ELSE clauses
            AnyNumberOf::new(vec_of_erased![
                one_of(vec_of_erased![
                    // ELSE IF
                    Sequence::new(vec_of_erased![
                        /* ELSE keyword */,
                        /* IF keyword */,
                        /* Expression */,
                        MetaSegment::indent(),
                        Ref::new("IfStatementsSegment"),
                        MetaSegment::dedent()
                    ]),
                    // Plain ELSE
                    Sequence::new(vec_of_erased![
                        /* ELSE keyword */,
                        MetaSegment::indent(),
                        Ref::new("IfStatementsSegment"),
                        MetaSegment::dedent()
                    ])
                ])
            ])
            .config(|this| this.optional())
        ])
    })
)]);
```

### 5. ElseStatementSegment Approach (User Requested)
- Implemented separate `ElseStatementSegment` and `ElseIfStatementSegment`
- Added these to the `WordAwareStatementSegment` list
- Result: Test passed but linter still showed ELSE indentation errors

### 6. Key Findings
1. **Parser structure is correct**: The dedent before ELSE ensures proper indentation level
2. **LT02 rule issue**: The problem appears to be in how LT02's `ReflowSequence::reindent()` processes the structure
3. **Two keywords vs one**: T-SQL uses "ELSE IF" (two keywords) while other dialects like BigQuery use "ELSEIF" (one keyword)
4. **Similar patterns work elsewhere**: BigQuery's IF statement has similar structure and works correctly

## Hypothesis
The issue is likely in the LT02 rule implementation, specifically how `ReflowSequence::reindent()` handles the two-keyword "ELSE IF" construct. The parser is generating the correct AST with proper indent/dedent markers, but the reindent logic may be misinterpreting the structure.

### 7. Implicit Indent Approach (Similar to CASE)
- Tried using `MetaSegment::implicit_indent()` similar to CASE statements
- Result: Still fails with same LT02 violations - ELSE keywords expected to be indented

### 8. BigQuery-like Pattern
- Reverted to BigQuery-like pattern with explicit indent/dedent
- Added dedent before ELSE clauses (matching BigQuery structure)
- Result: Still fails - ELSE keywords expected to be indented

### 9. CASE-like Pattern with Implicit Indent
- Tried using `implicit_indent` after IF keyword (similar to CASE structure)
- Removed all internal indent/dedent markers
- Result: Still fails - ELSE keywords expected to be indented

### Key Insight
The issue appears to be that LT02's indentation logic treats ELSE as a new statement that should be indented, rather than recognizing it as part of the IF statement structure at the same level. This is likely because:
1. T-SQL uses "ELSE IF" (two keywords) vs BigQuery's "ELSEIF" (one keyword)
2. The indent/dedent markers are correct in the parser
3. The LT02 rule's `ReflowSequence::reindent()` logic needs to handle this case specially

## Root Cause Analysis
After extensive testing, the issue is NOT with the parser structure. The parser correctly places indent/dedent markers. The problem is in the LT02 rule implementation:
1. The rule processes each line independently
2. When it encounters "ELSE" at the start of a line, it expects it to be indented based on the current indent level
3. It doesn't recognize that ELSE should be at the same level as IF in T-SQL

## Possible Solutions
1. **Modify LT02 rule**: Add special handling for T-SQL's two-keyword "ELSE IF" construct
2. **Use a different segment type**: Create a special segment type that LT02 recognizes as needing different indentation
3. **Update test expectations**: Accept that T-SQL ELSE statements should be indented (but this goes against SQL formatting standards)

## Next Steps
1. Debug the LT02 rule's reindent logic to understand how it processes T-SQL IF statements
2. Compare with how it handles single-keyword constructs like BigQuery's ELSEIF
3. Consider if the issue is related to how the parser segments "ELSE IF" as two separate tokens
4. Investigate if other T-SQL constructs with multiple keywords have similar issues
5. Look at how other dialects handle multi-part keywords (e.g., MySQL's "END IF")