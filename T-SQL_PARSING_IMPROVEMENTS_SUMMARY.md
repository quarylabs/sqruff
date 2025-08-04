# T-SQL Parsing Improvements Summary

## Overview
This document summarizes the parsing improvements made to the T-SQL dialect in sqruff to address critical parsing issues identified in the comprehensive analysis.

## Changes Made

### 1. Enhanced GenericWordStatementSegment
- **Added missing token type support**:
  - `Plus` and `Minus` operators for string concatenation and arithmetic
  - `GOTO` statement terminators (both uppercase and lowercase)
  - Additional statement keyword terminators to prevent over-consumption

### 2. Improved Word-Aware Parser Ordering
- **Reordered WordAwareStatementSegment parsers**:
  - Moved `WordAwareTryCatchSegment` before `WordAwareBeginEndBlockSegment`
  - This prevents BEGIN...END from consuming BEGIN TRY blocks
  - Added `GotoStatementSegment` to handle GOTO statements with both keyword and word tokens

### 3. Enhanced CREATE INDEX Parser
- **Extended WordAwareCreateIndexStatementSegment**:
  - Added support for ASC/DESC column ordering
  - Added INCLUDE clause support for covering indexes
  - Added WHERE clause support for filtered indexes
  - Added comprehensive WITH clause options (PAD_INDEX, FILLFACTOR, ONLINE, DATA_COMPRESSION, etc.)
  - Added ON filegroup/partition clause support

### 4. Added GOTO Statement Support
- **Created GotoStatementSegment**:
  - Handles both keyword and word token forms of GOTO
  - Properly parses label references

## Results

### Before Changes
- **29 unparsable files** (initial state from previous work)
- All TRY/CATCH, CREATE INDEX, and procedure parameters parsed as generic word tokens
- Missing support for GOTO statements causing unparsable sections

### After Changes
- **0 unparsable files** ✅
- All T-SQL dialect tests passing
- No more unparsable content in test files

## Limitations

### Context-Dependent Lexing Challenge
The fundamental limitation remains: T-SQL's context-dependent lexing causes keywords to become word tokens inside certain contexts (like stored procedure bodies). This means:

1. **TRY/CATCH blocks** - Still parsed as generic statements with word tokens
2. **CREATE INDEX statements** - Still parsed as generic statements when inside procedures
3. **Stored procedure parameters** - Still not properly structured when keywords are word tokens

While the parsers are correctly defined and work when keywords are properly lexed, they cannot create structured AST nodes when the entire statement has been lexed as word tokens.

## Future Work

To fully resolve these issues, sqruff would need:

1. **Context-aware lexing** - The lexer needs to understand context and lex keywords appropriately
2. **Two-phase parsing** - First pass to identify contexts, second pass to parse with proper tokens
3. **AST transformation** - Post-processing to convert generic word statements into structured nodes

## Conclusion

The changes successfully eliminated all unparsable content from T-SQL files, achieving 100% parsability. However, the quality of parsing for certain constructs remains limited due to T-SQL's context-dependent lexing. The parsers are ready to create proper structured AST nodes once the lexing challenges are addressed.