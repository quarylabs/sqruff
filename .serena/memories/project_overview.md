# Sqruff Project Overview

## Purpose
Sqruff is a fast SQL linter and formatter written in Rust, designed as a high-performance alternative to SQLFluff. It provides advanced, customizable SQL linting capabilities and automated formatting for SQL code consistency.

## Key Features
- **Linting**: Advanced, customizable SQL linting to ensure query quality
- **Formatting**: Automated, configurable formatting for SQL code consistency  
- **Speed**: Fast and efficient with minimal overhead
- **Portability**: Easily integrated into various development workflows
- **Multi-dialect**: Supports 13+ SQL dialects including ANSI, BigQuery, PostgreSQL, Snowflake, etc.

## Architecture
The project uses a Rust workspace structure with multiple crates:

### Core Crates
- **cli/**: Main CLI binary (sqruff)
- **cli-lib/**: CLI library with commands
- **lib/**: Core library with linting rules
- **lib-core/**: Core parsing and AST logic  
- **lib-dialects/**: SQL dialect implementations

### Integration Crates
- **cli-python/**: Python bindings via maturin
- **lib-wasm/**: WebAssembly bindings for browser support
- **lsp/**: Language Server Protocol implementation
- **lineage/**: SQL lineage analysis
- **sqlinference/**: SQL type inference

## Technology Stack
- **Primary Language**: Rust (edition 2024)
- **Python Bindings**: maturin for Python integration
- **Web Components**: TypeScript/React for playground
- **Browser Support**: WebAssembly compilation
- **Configuration**: TOML-based configuration files

## SQLFluff Relationship
Sqruff takes significant inspiration from SQLFluff and maintains compatibility where possible:
- Rule implementations follow SQLFluff patterns
- Test cases are imported from SQLFluff
- Dialect definitions mirror SQLFluff structure
- Configuration attempts to match SQLFluff behavior