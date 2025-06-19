# T-SQL Dialect Test Fixtures

This directory contains test fixtures for the T-SQL dialect implementation in sqruff.

## Files

- `tsql_basic.sql` - Basic T-SQL syntax that should parse correctly
- `tsql_known_issues.sql` - Known parsing issues that need to be fixed
- `*.yml` - Expected parse tree outputs (generated with `env UPDATE_EXPECT=1 cargo test`)

## Known Issues

1. **Variables in parentheses**: T-SQL variables (e.g., `@param`) cause parsing errors when used within parentheses. This affects:
   - `VALUES` clauses in INSERT statements
   - `WHERE` conditions with parentheses
   - Function arguments
   - Any expression within parentheses containing variables

## Running Tests

```bash
# Run all T-SQL dialect tests
cargo test dialect_tsql

# Update expected outputs after making changes
env UPDATE_EXPECT=1 cargo test dialect_tsql

# Run specific test
cargo test tsql_basic
```

## Adding New Tests

1. Add SQL test case to appropriate `.sql` file
2. Run `env UPDATE_EXPECT=1 cargo test` to generate expected output
3. Review the generated `.yml` file to ensure it's correct
4. Document any known issues or limitations in comments