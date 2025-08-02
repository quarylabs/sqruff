# T-SQL Unparsable Files - Root Cause Analysis

## Summary
4 unparsable files found, representing 2 distinct issues:

### Issue 1: Context-Dependent Keyword Lexing (2 files)
- `function_no_return.yml` - Keywords lexed as words after `AS` in procedures
- `try_catch.yml` - Keywords lexed as words after `THROW` statement

**Status**: Architectural limitation, cannot be easily fixed

### Issue 2: Join Hints Override Bug (2 files) 
- `join_hints.yml` - Join hints (HASH, LOOP, MERGE) not recognized
- `nested_joins.yml` - Same issue with nested joins

**Status**: Can be fixed! Bug found in the code

## Root Cause for Join Hints Issue

The T-SQL dialect correctly defines join hints support at line 4471:
```rust
// Line 4471: Proper definition with join hints
"TsqlJoinTypeKeywordsGrammar".into(),
Sequence::new(vec_of_erased![
    // Optional join type
    one_of(vec_of_erased![...]).config(|this| this.optional()),
    // Optional join hint (HASH, MERGE, LOOP)
    Ref::new("TsqlJoinHintGrammar").optional()
])
```

However, this is **overridden** at line 9631:
```rust
// Line 9631: Override that removes join hints!
"TsqlJoinTypeKeywordsGrammar".into(),
one_of(vec_of_erased![
    // Simple join types WITHOUT hints
    ...
])
```

## Fix
Remove or update the override at line 9631 to include join hints support.

## Impact
Fixing the join hints issue would improve the success rate from 97.48% to 98.74% (only 2 files would remain unparsable instead of 4).