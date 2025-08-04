# Sqruff Code Style and Conventions

## Rust Code Style

### General Guidelines
- Uses **Rust 2024 edition** (specified in Cargo.toml)
- Follows standard Rust formatting via `rustfmt`
- Code should be formatted using `cargo fmt --all` or `make rust_fmt`

### Linting Standards
The project enforces strict linting rules:

#### Workspace-level Lints (from Cargo.toml)
```toml
[workspace.lints.rust]
unreachable_pub = "warn"
unused_qualifications = "warn"

[workspace.lints.clippy]
perf = "warn"
cloned_instead_of_copied = "warn"
```

#### Enforced via CI
- `cargo clippy --all --all-features -- -D warnings` (warnings treated as errors)
- `cargo machete` (unused dependency detection)
- `cargo hack check --each-feature` (feature flag validation)

### Code Organization
- **Workspace Structure**: Multi-crate workspace with logical separation
- **Module Organization**: Clear separation between parsing, linting, formatting, and dialect-specific code
- **Error Handling**: Proper error propagation using `Result<T, E>` types

### Naming Conventions
- **Crates**: kebab-case (e.g., `lib-core`, `cli-python`)
- **Functions/Variables**: snake_case
- **Types/Structs**: PascalCase
- **Constants**: SCREAMING_SNAKE_CASE

## Python Code Style

### Tools and Standards
- **Formatter**: `ruff format` for code formatting
- **Linter**: `ruff check` for linting
- **Python Version**: Requires Python >=3.9

### Commands
```bash
make python_fmt    # Format Python code
make python_lint   # Lint Python code
```

## Testing Conventions

### Rust Tests
- Unit tests in `#[cfg(test)]` modules alongside code
- Integration tests in `tests/` directories
- Dialect tests use YAML fixtures in `test/fixtures/dialects/`

### Test Update Workflow
```bash
# Update test expectations after changes
env UPDATE_EXPECT=1 cargo test --no-fail-fast
```

### Test Naming
- Test functions use descriptive names with `test_` prefix
- Test files follow module structure

## Documentation Standards
- **Auto-generated Docs**: Some docs are auto-generated (cli.md, rules.md, templaters.md)
- **Never edit auto-generated files directly**
- Comments should explain "why" not "what"
- Use `///` for public API documentation
- Use `//` for internal comments

## SQLFluff Compatibility
When implementing new features:
1. **Research first**: Check SQLFluff's implementation
2. **Copy tests**: Use SQLFluff's test cases when applicable  
3. **Document differences**: Note intentional deviations from SQLFluff
4. **Maintain compatibility**: Ensure SQLFluff test cases pass