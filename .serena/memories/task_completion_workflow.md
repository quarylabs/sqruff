# Task Completion Workflow

## Essential Steps After Completing a Task

### 1. Code Quality Checks
```bash
# Format all code
make rust_fmt
make python_fmt

# Run all linting checks
make rust_lint
make python_lint
```

### 2. Testing
```bash
# Run all tests to ensure nothing is broken
make rust_test
make python_test

# For parser/dialect changes, update test fixtures if needed
env UPDATE_EXPECT=1 cargo test --no-fail-fast
```

### 3. Full CI Validation
```bash
# Run the complete CI suite
make ci
```

## Specific Workflows by Task Type

### Parser/Dialect Changes
1. **Prove parsing works**: Use dialect tests before changing rules
2. **Update fixtures**: `env UPDATE_EXPECT=1 cargo test --no-fail-fast`  
3. **Test SQLFluff compatibility**: Ensure SQLFluff test cases still pass
4. **Document changes**: Note any intentional differences from SQLFluff

### Rule Implementation
1. **Check SQLFluff first**: Review SQLFluff's implementation
2. **Import tests**: Copy SQLFluff test cases as starting point
3. **Add to rule registry**: Update `crates/lib/src/rules/mod.rs`
4. **Test thoroughly**: Include SQL examples with edge cases

### New Dialect Support
1. **Research SQLFluff**: Check if dialect exists in SQLFluff
2. **Copy implementation**: Start with SQLFluff's dialect definition
3. **Import tests**: Add to `crates/lib/test/fixtures/dialects/`
4. **Update documentation**: Add to dialect list in README.md

## Error Resolution Workflow

### Compilation Errors
```bash
# Check compilation issues
cargo check

# Fix formatting issues
make rust_fmt

# Address clippy warnings
cargo clippy --all --all-features -- -D warnings
```

### Test Failures
```bash
# Run specific failing test with output
cargo test <test_name> --no-fail-fast -- --nocapture

# Update test expectations if changes are intentional
env UPDATE_EXPECT=1 cargo test <test_name>

# Run all tests to check for regressions
cargo test --no-fail-fast
```

## Pre-Commit Checklist
- [ ] Code is formatted (`make rust_fmt`, `make python_fmt`)
- [ ] All linting passes (`make rust_lint`, `make python_lint`)
- [ ] All tests pass (`make rust_test`, `make python_test`)
- [ ] Test fixtures updated if needed (`UPDATE_EXPECT=1`)
- [ ] Changes documented if they affect public API
- [ ] SQLFluff compatibility maintained where applicable

## Release Preparation
1. Update version numbers in relevant Cargo.toml files
2. Run full test suite
3. Update documentation if needed
4. Commit changes with descriptive message
5. Tag commit with version number