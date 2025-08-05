# T-SQL Word-Aware Parsing Clarification

## Executive Summary

**The word-aware parsing is working correctly and creates proper nested AST structures.** The confusion arose from misinterpreting the YAML test file format, which intentionally shows only code tokens without structural metadata.

## Key Findings

### 1. Word-Aware Parsers Create Proper Structure

The word-aware parsers like `WordAwareBeginEndBlockSegment` correctly include indentation metadata:

```rust
Sequence::new(vec_of_erased![
    StringParser::new("BEGIN", SyntaxKind::Word),
    MetaSegment::indent(),  // ← Creates indentation structure
    AnyNumberOf::new(vec_of_erased![
        Ref::new("WordAwareStatementSegment")
    ]),
    MetaSegment::dedent(),  // ← Closes indentation structure
    StringParser::new("END", SyntaxKind::Word)
])
```

### 2. Indentation Rules Work Correctly

Evidence that proper AST structure exists:

```sql
-- Input with bad indentation
CREATE PROCEDURE dbo.TestIndentationCheck
AS
BEGIN
IF @test = 1  -- No indentation
BEGIN
SELECT * FROM table1;  -- Wrong indentation
END
END;

-- After sqruff fix (LT02 rules applied correctly)
CREATE PROCEDURE dbo.testindentationcheck
AS
BEGIN
    IF @test = 1      -- ✓ Correctly indented
    BEGIN
        SELECT * FROM table1;  -- ✓ Correctly indented
    END
END;
```

The LT02 indentation rules can only work if the AST has proper nesting structure.

### 3. YAML Test Files Are Misleading

The YAML test files use `to_serialised(true, true)` where the first `true` means `code_only`:

```rust
// From dialects.rs test
let tree = tree.to_serialised(true, true);  // code_only=true filters out metadata

// From segments.rs
if code_only {
    let segments = self
        .segments()
        .iter()
        .filter(|seg| seg.is_code() && !seg.is_meta())  // ← Filters out indentation markers
        .map(|seg| seg.to_serialised(code_only, show_raw))
        .collect::<Vec<_>>();
}
```

This filtering removes:
- `MetaSegment::indent()` markers
- `MetaSegment::dedent()` markers
- Other structural metadata

## Why This Matters

### 1. T-SQL Parsing Is More Mature Than Previously Thought
- Word-aware parsers successfully handle context-dependent lexing
- Proper AST structures enable correct formatting and linting
- The architecture supports T-SQL's unique requirements

### 2. YAML Test Files Should Not Be Used to Judge AST Structure
- They show a simplified view for testing token sequences
- The actual AST contains rich structural information
- Use formatting behavior and rule application as evidence of proper structure

### 3. Future Development Can Build on Solid Foundation
- New word-aware parsers can follow the existing pattern
- Indentation and formatting will work correctly
- No major architectural changes needed

## Recommendations

1. **For Developers**: When implementing new word-aware parsers, ensure they include proper `MetaSegment::indent()`/`dedent()` markers

2. **For Testing**: Don't rely solely on YAML files to verify AST structure. Instead:
   - Run formatting tests to verify indentation
   - Check that layout rules (LT02, LT04) work correctly
   - Use debugger to inspect actual AST structure if needed

3. **For Documentation**: Update any documentation that suggests T-SQL parsing creates flat structures

## Conclusion

The T-SQL word-aware parsing implementation is fundamentally sound. The perceived "flattening" issue was actually a misunderstanding of how test fixtures display parsed results. The system correctly handles T-SQL's context-dependent lexing while maintaining proper AST structure for formatting and linting.