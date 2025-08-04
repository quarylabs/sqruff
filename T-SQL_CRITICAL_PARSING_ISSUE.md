# CRITICAL T-SQL Parsing Issue: No Nested Structure

## The Problem

While we achieved 100% parsability (no unparsable content), the parsing quality is severely compromised. The word tokens are NOT being parsed into proper nested AST structures.

### Evidence

1. **YAML Test Files Show Flat Token Streams**
```yaml
- word: BEGIN
- word: IF
- word: SELECT
- word: END
```
No nested structure, just flat word tokens.

2. **Formatting Loses All Indentation**
When running `sqruff fix` on nested procedures:
- All indentation is removed
- Nested BEGIN...END blocks become flat
- IF/ELSE structures lose their hierarchy

### Impact on Rules

This breaks multiple rule categories:

1. **Layout Rules (LT02, LT04)** - Cannot determine proper indentation without AST structure
2. **Structure Rules** - Cannot validate proper nesting
3. **Complexity Rules** - Cannot measure nesting depth
4. **Many Other Rules** - Depend on understanding code structure

## Root Cause Analysis

### 1. Context-Dependent Lexing
T-SQL lexes keywords as word tokens inside procedural contexts. This is by design and cannot be changed without major architectural changes.

### 2. Word-Aware Parsers Not Creating Structure
While we created word-aware parsers like:
- `WordAwareBeginEndBlockSegment`
- `WordAwareTryCatchSegment`
- `WordAwareIfStatementSegment`

They are either:
- Not matching correctly due to token type mismatches
- Not being invoked in the right order
- Being overridden by the generic word statement parser

### 3. Test Expectations Show Token Stream
The YAML test files might be showing the raw token stream rather than the parsed AST structure, making it hard to verify if parsing is working.

## Potential Solutions

### Short-term (High Effort)
1. Debug why word-aware parsers aren't creating nested structures
2. Ensure parser ordering prioritizes structured parsers over generic ones
3. Add explicit structure creation in word-aware parsers

### Long-term (Very High Effort)
1. Implement context-aware lexing that preserves keywords in certain contexts
2. Add a pre-processing phase to identify and mark statement boundaries
3. Post-process generic word statements to reconstruct structure

## Recommendation

This is a CRITICAL issue that severely limits sqruff's ability to properly lint and format T-SQL code. Without proper AST structure:
- Indentation rules don't work
- Code structure validation fails
- The formatter produces incorrect output

The word-aware parsing approach needs significant enhancement to create proper nested structures from word tokens, or T-SQL support will remain very limited.