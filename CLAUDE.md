# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sqruff is a fast SQL linter and formatter written in Rust with Python bindings. It's designed as a high-performance alternative to sqlfluff.

### Key Technologies
- **Rust** (primary language) with workspace structure
- **Python** bindings via maturin
- **TypeScript/React** for web playground
- **WebAssembly** for browser support

## Development Setup

### Prerequisites
```bash
# Install Rust toolchain (version in rust-toolchain.toml)
rustup install stable

# Set up Python environment
virtualenv .venv
source .venv/bin/activate
make python_install

# Install Node.js dependencies (requires pnpm 9+)
pnpm install

# Load VS Code settings
make load_vscode_settings
```

## Common Development Commands

### Building and Running
```bash
# Build the project
cargo build

# Run sqruff CLI
cargo run -- lint <file.sql>
cargo run -- fix <file.sql>

# Run with specific dialect
cargo run -- lint --dialect snowflake <file.sql>
```

### Testing
```bash
# Run all Rust tests
make rust_test

# Run specific test
cargo test <test_name> --no-fail-fast

# Run Python tests
make python_test

# Update dialect test fixtures (YAML files)
env UPDATE_EXPECT=1 cargo test --no-fail-fast

# Run single test with output
cargo test <test_name> --no-fail-fast -- --nocapture

# Run all tests and see all failures
cargo test --no-fail-fast
```

### Code Quality
```bash
# Format Rust code
make rust_fmt

# Lint Rust code (includes clippy, machete, hack)
make rust_lint

# Format Python code
make python_fmt

# Lint Python code
make python_lint

# Run all CI checks
make ci
```

## Architecture

### Workspace Structure
```
crates/
├── cli/              # Main CLI binary (sqruff)
├── cli-lib/          # CLI library with commands
├── cli-python/       # Python bindings (maturin)
├── lib/              # Core library with linting rules
├── lib-core/         # Core parsing and AST logic
├── lib-dialects/     # SQL dialect implementations
├── lib-wasm/         # WebAssembly bindings
├── lsp/              # Language Server Protocol
├── lineage/          # SQL lineage analysis
└── sqlinference/     # SQL type inference
```

### Key Components

1. **Parser (`lib-core`)**: Converts SQL text into an Abstract Syntax Tree (AST)
2. **Rules Engine (`lib`)**: Applies linting rules to the AST
3. **Formatter (`lib`)**: Transforms AST back to formatted SQL
4. **Dialects (`lib-dialects`)**: Dialect-specific parsing rules
5. **CLI (`cli` + `cli-lib`)**: Command-line interface
6. **LSP (`lsp`)**: Language server for editor integration

### Rule System
- Rules are defined in `crates/lib/src/rules/`
- Each rule is a struct implementing the `Rule` trait
- Rules can be enabled/disabled via `.sqruff` configuration
- Rules follow a naming convention (e.g., AL01, CP02)

## SQLFluff Relationship

Sqruff takes significant inspiration from SQLFluff. Understanding when to follow SQLFluff is crucial:

### When to Follow SQLFluff (Default Approach)
- **Rules**: Check SQLFluff's rule implementations first (https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/rules)
- **Dialects**: Copy dialect definitions and tests from SQLFluff when possible
- **Testing**: Use SQLFluff's test cases - copying is faster than creating from scratch
- **Behavior**: Match SQLFluff's behavior for rules and parsing to ensure compatibility

### When to Diverge from SQLFluff
- **Configuration format**: Sqruff uses INI format instead of SQLFluff's YAML
- **Core linter structure**: Rust implementation allows different architectural choices
- **Performance optimizations**: Take advantage of Rust's capabilities
- **Extended dialect features**: Not limited by SQLFluff's implementations (e.g., SQL Server 2017+ syntax)

### Development Approach
1. **Research first**: Always check SQLFluff's implementation before starting
2. **Copy tests**: Use SQLFluff's test cases, especially for dialects
3. **Document extensions**: Clearly note where Sqruff intentionally extends beyond SQLFluff
4. **Maintain compatibility**: Ensure SQLFluff test cases pass in Sqruff

### Testing Strategy
- **Cautious approach**: Due to past regressions, be extra careful with changes
- **Prove parsing first**: Use dialect tests to prove parsing works before changing rules
- **Copy SQLFluff tests**: Especially for dialect features and edge cases
- **Dialect tests**: YAML fixtures in `crates/lib/test/fixtures/dialects/`
- **Unit tests**: Alongside code (`#[cfg(test)]` modules)
- **Integration tests**: In `tests/` directories
- **UI tests**: For CLI output validation

## Auto-Generated Documentation

**IMPORTANT**: Some documentation files are auto-generated from source code. Never edit these files directly:

- **`docs/cli.md`** - Generated from CLI argument definitions using clap-markdown
- **`docs/rules.md`** - Generated from rules source code
- **`docs/templaters.md`** - Generated from templaters source code

To update these docs:
1. Modify the source code (CLI args, rule implementations, or templater code)
2. The GitHub Actions workflow will automatically regenerate the docs
3. Alternatively, run locally: `cargo run --bin sqruff -F codegen-docs`

## Configuration

### `.sqruff` File Format
```ini
[sqruff]
dialect = snowflake
exclude_rules = AM01,AM02
rules = all

[sqruff:indentation]
indent_unit = space
tab_space_size = 4
```

### NoQA Directives
```sql
-- Ignore all errors on this line
SELECT * FROM table; -- noqa

-- Ignore specific rules
SELECT * FROM table; -- noqa: AL01,AL02

-- Disable rules for a range
-- noqa: disable=all
SELECT * FROM table;
-- noqa: enable=all
```

## Adding New Features

### New Linting Rule
1. **Check SQLFluff first**: Look at SQLFluff's implementation (https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/rules)
2. Create new rule file in `crates/lib/src/rules/`
3. Implement the `Rule` trait
4. **Copy SQLFluff tests**: Use their test cases as a starting point
5. Add to rule registry in `crates/lib/src/rules/mod.rs`
6. Add tests with SQL examples (including SQLFluff compatibility tests)
7. Note any intentional differences from SQLFluff

### New SQL Dialect
1. **Research SQLFluff dialect**: Check if SQLFluff has this dialect (https://github.com/sqlfluff/sqlfluff/tree/main/src/sqlfluff/dialects)
2. **Copy SQLFluff implementation**: If available, start by copying their dialect definition and tests
3. Create dialect module in `crates/lib-dialects/src/`
4. Define grammar rules and keywords (referencing SQLFluff where applicable)
5. **Copy SQLFluff dialect tests**: Add to `crates/lib/test/fixtures/dialects/`
6. Add additional tests for any Sqruff-specific extensions
7. Update dialect list in README.md
8. Document any extensions beyond SQLFluff's implementation

## Debugging Tips

### Debugging Linting Issues (When Rules Flag Valid Syntax)

**IMPORTANT**: When a linting rule incorrectly flags valid syntax, always check the parser first before modifying the rule. Most issues stem from incorrect AST generation, not rule logic.

#### Debugging Approach
1. **Parser First**: Check if the parser creates the correct AST structure
   - Create minimal test cases to isolate the issue
   - Test components separately (e.g., `TOP` alone, `DISTINCT` alone, then `DISTINCT TOP`)
   - Look for "unparsable" segments in the AST - this indicates parser grammar issues
2. **Grammar Combinations**: SQL dialects often support keyword combinations that need explicit handling
   - Example: T-SQL's `DISTINCT TOP` needs to be defined as a combined modifier
   - Check if `SelectClauseModifierSegment` or similar grammar rules handle all valid combinations
3. **AST Node Types**: Ensure semantically correct node types
   - Example: T-SQL's `alias = expression` should use `AssignmentOperator`, not `ComparisonOperator`
   - Wrong node types can confuse rules and lead to incorrect behavior
4. **Only Then Check Rules**: If parser output is correct, then investigate rule logic

#### Example: T-SQL DISTINCT TOP Issue
```sql
-- This valid T-SQL was incorrectly flagged by AL02:
SELECT DISTINCT TOP 20 JiraIssueID = JiraIssue.i_jira_id
```
**Root Cause**: The T-SQL dialect's `SelectClauseModifierSegment` didn't support `DISTINCT TOP` as a combined modifier. The parser created separate modifiers, breaking the subsequent T-SQL alias pattern matching.

**Fix**: Updated the grammar to explicitly handle the combination:
```rust
// Support DISTINCT/ALL followed by TOP as a single modifier
Sequence::new(vec_of_erased![
    one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")]),
    Ref::keyword("TOP"),
    // ... rest of TOP grammar
])
```

### Parser Issues
- Use `cargo test <test_name> -- --nocapture` to see parse tree output
- Check dialect-specific grammar in `crates/lib-dialects/`
- Update test fixtures with `env UPDATE_EXPECT=1 cargo test`
- Create minimal SQL test cases to isolate parser behavior
- Look for patterns where valid syntax creates "unparsable" segments

### Rule Development
- Use `sqruff parse <file.sql>` to see AST structure
- Add `println!` debugging in rule implementation
- Test rules with minimal SQL examples first
- Verify the parser creates expected AST before implementing rule logic

## Release Process
1. Update versions in Cargo.toml files
2. Commit and push changes
3. Tag with version (e.g., `v0.1.0`)
4. Publish crates: `cargo publish -p sqruff-lib && cargo publish -p sqruff`
5. Python package is published automatically via GitHub Actions