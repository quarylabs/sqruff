# T-SQL Parsing Investigation Report

## Executive Summary

While we successfully eliminated all unparsable content in T-SQL files (29 → 0 files), a critical issue remains: **T-SQL code is not being parsed into proper nested AST structures**. This severely impacts the linter's ability to enforce layout rules, validate code structure, and format code correctly.

## Investigation Findings

### 1. Current State

#### ✅ What's Working
- **100% Parsability**: All T-SQL files parse without errors
- **No Unparsable Content**: GenericWordStatementSegment enhancements resolved all parsing failures
- **Basic Linting**: Simple rules (like capitalization) still function
- **Word-Aware Parsers Created**: Infrastructure exists for handling word tokens

#### ❌ What's Not Working
- **No AST Structure**: Code is parsed as flat sequences of word tokens
- **Broken Indentation**: Layout rules (LT02, LT04) cannot determine proper nesting
- **Failed Formatting**: The fix command removes all indentation, flattening nested structures
- **Limited Rule Enforcement**: Structure-dependent rules cannot function

### 2. Evidence of the Problem

#### Test Case 1: Simple Nested Procedure
```sql
CREATE PROCEDURE dbo.TestNesting
AS
BEGIN
    IF @test = 1
    BEGIN
        SELECT * FROM table1;
    END
END;
```

**Expected**: Nested structure with proper indentation
**Actual**: All indentation removed after formatting

#### Test Case 2: Complex Nesting
```sql
-- Original
BEGIN
    WHILE @i < 10
    BEGIN
        IF @i % 2 = 0
        BEGIN
            BEGIN TRY
                SELECT @i;
            END TRY
            BEGIN CATCH
                PRINT ERROR_MESSAGE();
            END CATCH
        END
    END
END

-- After sqruff fix (all indentation lost)
BEGIN
WHILE @i < 10
BEGIN
IF @i % 2 = 0
BEGIN
BEGIN TRY
SELECT @i;
END TRY
BEGIN CATCH
PRINT ERROR_MESSAGE();
END CATCH
END
END
END
```

#### YAML Test Output
```yaml
- word: BEGIN
- word: IF
- word: SELECT
- word: END
# No nested structure, just flat tokens
```

### 3. Root Cause Analysis

#### A. T-SQL's Context-Dependent Lexing
- Inside procedural contexts, T-SQL lexes keywords as regular identifiers
- This produces `word` tokens instead of keyword tokens
- This is by design to allow keywords as identifiers

#### B. Parser Matching Issues
The word-aware parsers we created have several issues:

1. **Token Type Mismatch**: Some parsers look for `SyntaxKind::Keyword` when they should match `SyntaxKind::Word`
2. **Parser Priority**: Generic word statement parser may be consuming tokens before structured parsers
3. **Missing Structure Creation**: Parsers match patterns but don't create proper AST nodes

#### C. Test Infrastructure Limitations
- YAML files show raw token streams, not parsed AST structure
- Makes it difficult to verify if parsing improvements are working

### 4. Impact Assessment

#### Affected Functionality
| Feature | Impact | Severity |
|---------|---------|----------|
| Indentation Rules | Completely broken | CRITICAL |
| Code Formatting | Produces incorrect output | CRITICAL |
| Structure Validation | Cannot detect improper nesting | HIGH |
| Complexity Analysis | Cannot measure nesting depth | MEDIUM |
| Navigation | Cannot traverse code structure | MEDIUM |

#### Affected Rules
- **LT02**: Line indentation
- **LT04**: Indentation consistency
- **ST05**: Structure validation
- **CP01**: Complexity measurements
- Many others that depend on AST structure

### 5. Technical Deep Dive

#### Current Parser Flow
1. Lexer produces word tokens for keywords in procedural contexts
2. Word-aware parsers attempt to match patterns
3. Generic word statement parser consumes remaining tokens
4. Result: Flat sequence of statements without structure

#### Example: WordAwareBeginEndBlockSegment
```rust
StringParser::new("BEGIN", SyntaxKind::Word),
MetaSegment::indent(),
AnyNumberOf::new(vec_of_erased![
    Ref::new("WordAwareStatementSegment")
]),
MetaSegment::dedent(),
StringParser::new("END", SyntaxKind::Word)
```

This parser includes indentation markers but may not be matching correctly.

## Recommendations

### Immediate Actions (1-2 weeks)
1. **Fix Token Type Mismatches**: Audit all word-aware parsers to ensure they match `SyntaxKind::Word`
2. **Debug Parser Matching**: Add logging to understand which parsers are being invoked
3. **Verify Parser Priority**: Ensure structured parsers run before generic fallbacks

### Short-term Solutions (2-4 weeks)
1. **Enhanced Word-Aware Parsers**: Ensure they create proper AST nodes with structure
2. **Parser Ordering Optimization**: Carefully order parsers in WordAwareStatementSegment
3. **Test Infrastructure**: Create tests that verify AST structure, not just token streams

### Long-term Solutions (1-3 months)
1. **Context-Aware Lexing**: Modify lexer to preserve keywords in certain contexts
2. **Two-Phase Parsing**: First identify structure, then parse with context
3. **AST Post-Processing**: Transform flat structures into nested AST after initial parsing

## Conclusion

The current T-SQL implementation achieves parsing without errors but fails to create the structured AST required for proper linting and formatting. This is a **CRITICAL** issue that severely limits sqruff's T-SQL support.

Without addressing this issue:
- T-SQL formatting will remain broken
- Layout rules will not function
- Code structure validation is impossible
- User experience will be severely degraded

**Recommendation**: Prioritize fixing the word-aware parsers to create proper nested structures. This is essential for T-SQL to be a viable dialect in sqruff.