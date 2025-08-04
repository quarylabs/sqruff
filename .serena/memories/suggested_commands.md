# Essential Sqruff Development Commands

## Development Setup
```bash
# Install Rust toolchain (version specified in rust-toolchain.toml)
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

## Building and Running
```bash
# Build the project
cargo build

# Run sqruff CLI
cargo run -- lint <file.sql>
cargo run -- fix <file.sql>

# Run with specific dialect
cargo run -- lint --dialect snowflake <file.sql>
```

## Testing Commands
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

## Code Quality Commands
```bash
# Format Rust code
make rust_fmt
# Equivalent: cargo fmt --all

# Lint Rust code (includes clippy, machete, hack)
make rust_lint

# Format Python code
make python_fmt

# Lint Python code  
make python_lint

# Run all CI checks
make ci
```

## Development Workflow Commands
```bash
# Check compilation without building
cargo check

# Build for release
cargo build --release

# Install from source
cargo install --path crates/cli

# Run sqruff directly after build
./target/debug/sqruff --help
```

## Common System Commands (Linux)
```bash
# File operations
ls -la          # List files with details
find . -name "*.rs"  # Find Rust files
grep -r "pattern" .  # Search in files

# Git operations
git status
git add .
git commit -m "message"
git push

# Process management
ps aux | grep sqruff    # Find sqruff processes
killall sqruff         # Kill all sqruff processes
```