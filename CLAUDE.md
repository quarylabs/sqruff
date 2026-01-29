# CLAUDE.md

Sqruff is a fast SQL linter and formatter written in Rust. It's a high-performance alternative to SQLFluff.

## Quick Start

```bash
# Build and run
cargo build
cargo run -- lint <file.sql>
cargo run -- fix <file.sql>

# Run tests
cargo test --no-fail-fast

# Update test fixtures
env UPDATE_EXPECT=1 cargo test --no-fail-fast

# Format code
cargo fmt --all

# Run all lint checks via Bazel (clippy, rustfmt, prettier, ruff, machete)
bazel test //:rustfmt_check //:clippy_check //:prettier_check //:ruff_check //:cargo_machete
```

## Project Structure

```
crates/
├── cli/          # CLI binary
├── lib/          # Core linting rules
├── lib-core/     # Parser and AST
├── lib-dialects/ # SQL dialect implementations
├── lsp/          # Language Server Protocol
└── lib-wasm/     # WebAssembly bindings
```

## SQLFluff Compatibility

Sqruff is designed to be compatible with SQLFluff:
- **Check SQLFluff first** when implementing rules or dialects
- **Copy SQLFluff tests** - use their test cases as starting points
- Rules are in `crates/lib/src/rules/`, dialects in `crates/lib-dialects/src/`

## Configuration

`.sqruff` file (INI format):
```ini
[sqruff]
dialect = snowflake
exclude_rules = AM01,AM02
```

## Auto-Generated Docs

Do not edit directly - regenerate with `cargo run --bin sqruff -F codegen-docs`:
- `docs/reference/cli.md`
- `docs/reference/dialects.md`
- `docs/reference/rules.md`
- `docs/reference/templaters.md`
