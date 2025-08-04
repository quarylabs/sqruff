# Development Setup and Architecture

## System Requirements
- **Operating System**: Linux (current development environment)
- **Rust**: Version specified in `rust-toolchain.toml` (managed by rustup)
- **Python**: >=3.9 for Python bindings and tooling
- **Node.js**: Required for pnpm package manager (9+)

## Development Environment Setup

### Initial Setup
```bash
# Install Rust toolchain
rustup install stable

# Set up Python virtual environment
virtualenv .venv
source .venv/bin/activate
make python_install

# Install Node.js dependencies
pnpm install

# Configure VS Code (optional)
make load_vscode_settings
```

## Codebase Architecture

### Workspace Structure
```
sqruff/
├── crates/                   # Rust workspace crates
│   ├── cli/                 # Main CLI binary
│   ├── cli-lib/             # CLI command implementations
│   ├── cli-python/          # Python bindings (maturin)
│   ├── lib/                 # Core linting rules engine
│   ├── lib-core/            # Core parsing and AST logic
│   ├── lib-dialects/        # SQL dialect implementations
│   ├── lib-wasm/            # WebAssembly bindings
│   ├── lsp/                 # Language Server Protocol
│   ├── lineage/             # SQL lineage analysis
│   └── sqlinference/        # SQL type inference
├── docs/                    # Documentation (some auto-generated)
├── playground/              # Web playground (TypeScript/React)
├── tests/                   # Integration tests
└── test/                    # Test fixtures and data
```

### Key Components

#### 1. Parser (lib-core)
- Converts SQL text into Abstract Syntax Tree (AST)
- Handles lexical analysis and tokenization
- Core parsing logic shared across all dialects

#### 2. Rules Engine (lib)
- Applies linting rules to the AST
- Rule definitions following SQLFluff patterns
- Configurable rule enabling/disabling

#### 3. Formatter (lib)
- Transforms AST back to formatted SQL
- Handles indentation, spacing, and style

#### 4. Dialects (lib-dialects)
- Dialect-specific parsing rules and grammar
- Support for 13+ SQL dialects
- Extends base ANSI SQL with dialect-specific features

#### 5. CLI (cli + cli-lib)
- Command-line interface for end users
- Subcommands: lint, fix, format, parse
- Configuration file handling

## Configuration System
- **Main config**: `.sqruff` files (INI format)
- **Rust config**: `Cargo.toml` workspace configuration
- **Python config**: `pyproject.toml` for Python bindings
- **VS Code**: Shared settings via `make load_vscode_settings`

## Build System
- **Primary**: Cargo for Rust compilation
- **Python bindings**: maturin for Python wheel generation
- **Web**: pnpm for TypeScript/React playground
- **Automation**: Makefile for common development tasks

## Testing Architecture
- **Unit tests**: Embedded in source files (`#[cfg(test)]`)
- **Integration tests**: Separate `tests/` directories
- **Dialect tests**: YAML fixtures in `test/fixtures/dialects/`
- **Python tests**: pytest for Python binding validation
- **UI tests**: CLI output validation

## SQLFluff Relationship
The architecture deliberately mirrors SQLFluff's design:
- Rule system follows SQLFluff patterns
- Dialect structure matches SQLFluff organization  
- Test cases imported from SQLFluff for compatibility
- Configuration attempts to maintain SQLFluff compatibility