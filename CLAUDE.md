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

# Run key lint checks via Bazel
bazel test //:rustfmt_check //:clippy_check //:prettier_check //:ruff_check //:cargo_machete

# Hermetic cargo checks (vendored deps, isolated Rust toolchain)
bazel test //:cargo_check //:cargo_clippy //:cargo_fmt_check

# Run all tests via Bazel
bazel test //:cargo_test

# Verify generated docs are up to date
bazel test //:codegen_docs_check
```

## Project Structure

```
crates/
├── cli/           # CLI binary
├── cli-lib/       # Shared CLI library
├── cli-python/    # Python bindings (PyO3)
├── lib/           # Core linting rules
├── lib-core/      # Parser and AST
├── lib-dialects/  # SQL dialect implementations
├── lib-wasm/      # WebAssembly bindings
├── lineage/       # SQL lineage tracking
├── lsp/           # Language Server Protocol
└── sqlinference/  # SQL inference library
playground/        # React/TypeScript web playground (WASM-based)
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
- `docs/reference/rules.md`
- `docs/reference/templaters.md`
