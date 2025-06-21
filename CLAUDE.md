# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sqruff is a high-speed SQL linter and formatter written in Rust, inspired by SQLFluff and Ruff. It supports 13+ SQL dialects and provides comprehensive linting with 50+ rules across categories like Aliasing, Capitalisation, Convention, Layout, References, and Structure.

## Architecture

### Rust Workspace Structure (crates/)
- **`lib/`** - Core linting engine, rules, and templating logic
- **`lib-core/`** - Parser, lexer, and grammar infrastructure
- **`lib-dialects/`** - SQL dialect implementations (ANSI, BigQuery, PostgreSQL, etc.)
- **`cli/`** - Native Rust CLI binary
- **`cli-lib/`** - Shared CLI functionality
- **`cli-python/`** - Python bindings via PyO3
- **`lib-wasm/`** - WebAssembly bindings for browser usage
- **`lsp/`** - Language Server Protocol implementation
- **`lineage/`** - SQL lineage analysis
- **`sqlinference/`** - SQL schema inference

### Frontend Components
- **`playground/`** - Web-based SQL playground (TypeScript/React)
- **`editors/code/`** - VS Code extension

### Key Dependencies
- Rust 2024 edition with specific toolchain in `rust-toolchain.toml`
- Node.js ≥22.0.0 and pnpm ≥9.0.0 for frontend components
- Python for optional bindings

## Development Commands

### Rust Development
```bash
# Core development
cargo test                    # Run all tests
cargo fmt --all              # Format code
cargo clippy --all           # Lint code
env UPDATE_EXPECT=1 cargo test # Update test fixtures

# Via Makefile
make rust_test               # Run Rust tests
make rust_fmt               # Format Rust code
make rust_lint              # Lint Rust code (includes clippy, machete, hack)
make ci                     # Full CI pipeline
```

### Python Development
```bash
make python_test            # Run Python tests with pytest
make python_fmt             # Format Python code with ruff
make python_lint            # Lint Python code with ruff
make python_install         # Install dev dependencies
make python_ci              # Full Python CI
```

### Frontend Development
```bash
# pnpm workspace commands
pnpm run build              # Build all components
pnpm run test               # Run tests
pnpm run lint               # Lint all code
pnpm run fmt                # Format with prettier
pnpm run ci                 # Full frontend CI
```

### Testing Patterns

#### Fixture-Based Testing
- SQL parsing tests use fixture files in `crates/lib/test/fixtures/dialects/`
- Each SQL file has a companion YAML file with expected parsing output
- Update fixtures with `env UPDATE_EXPECT=1 cargo test`

#### Rule Testing
- Rules are tested via YAML test cases in `crates/lib/test/fixtures/rules/std_rule_cases/`
- Each rule (AL01-AL09, AM01-AM07, CP01-CP05, CV01-CV11, LT01-LT13, RF01-RF06, ST01-ST09) has comprehensive test coverage

#### VS Code Extension Development
```bash
# In editors/code directory
npm run build:wasm_lsp && npm run compile && npm run run-in-browser
```

## Rule System Architecture

### Rule Categories
- **Aliasing (AL)** - Table/column aliasing standards
- **Ambiguous (AM)** - Ambiguity prevention
- **Capitalisation (CP)** - Keyword/identifier casing
- **Convention (CV)** - SQL coding conventions
- **Layout (LT)** - Spacing, indentation, line formatting
- **References (RF)** - Identifier reference standards
- **Structure (ST)** - SQL structure and organization

### Rule Implementation
- Each rule is implemented as a separate module
- Rules use AST crawlers for traversal patterns
- Rules can provide automatic fixes
- NoQA support for inline rule disabling

## Configuration

### Default Configuration
- Main config file: `crates/lib/src/core/default_config.cfg`
- User config file: `.sqruff` in project root
- Ignore patterns: `.sqruffignore` file

### Supported Dialects
ANSI SQL (default), BigQuery, Athena, ClickHouse, Databricks, DuckDB, MySQL, PostgreSQL, Redshift, Snowflake, SparkSQL, SQLite, Trino, T-SQL

## Distribution Strategy

- **Native binary**: Fast Rust executable
- **Python package**: PyO3 bindings for pip install
- **VS Code extension**: LSP integration
- **Web playground**: WASM-powered browser experience
- **GitHub Action**: CI/CD integration
- **Homebrew**: macOS package manager

## CLI Usage Patterns

```bash
sqruff lint <file/directory>     # Lint SQL files
sqruff fix <file/directory>      # Fix SQL files
sqruff --help                    # Show help
```

## Development Utilities

### VS Code Setup
```bash
make load_vscode_settings        # Load sample VS Code settings from .hacking/vscode/
```

### GitHub Actions
```bash
make ratchet_pin                 # Pin workflow versions
make ratchet_update              # Update workflow versions
make ratchet_check               # Check workflow versions
```

## Templating Support

sqruff supports multiple templating engines:
- Raw SQL (default)
- Jinja templating
- dbt templating
- Python templating

Configuration and details available in `docs/templaters.md`.

## Important Notes

- Multi-crate workspace with version synchronization across crates
- Extensive CI/CD with cross-platform support
- Performance-optimized with LTO and codegen settings
- Uses ahash for performance-critical hashing
- WebAssembly support requires specific optimization settings

## Development Best Practices

- **Testing**: 
  - Always add a testcase when fixing an issue with a linter, to ensure it will work in the future