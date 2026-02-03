#!/bin/bash
# Run cargo test exactly like the GitHub Action does.
set -eo pipefail

# Create a temp directory and copy everything there (symlinks don't work with maturin)
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -rL "$RUNFILES_DIR/_main/." "$WORKDIR/"

cd "$WORKDIR"

# Use Bazel-provided uv - add its directory to PATH so maturin can find it
UV_BIN="$RUNFILES_DIR/$UV"
UV_DIR=$(dirname "$UV_BIN")
export PATH="$UV_DIR:$PATH"

# Isolate cargo build artifacts/registry to avoid conflicts with other parallel Bazel tests
export CARGO_HOME="$WORKDIR/.cargo"
export CARGO_TARGET_DIR="$WORKDIR/target"

# Use system cargo if available, otherwise install rustup
if ! command -v cargo &> /dev/null; then
    export RUSTUP_HOME="$WORKDIR/.rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --no-modify-path
    export PATH="$CARGO_HOME/bin:$PATH"
fi

# Install Python 3.12 via uv (includes development libraries for linking)
"$UV_BIN" python install 3.12

# Create virtual environment using uv-managed Python
"$UV_BIN" venv --python 3.12 .venv
source .venv/bin/activate

# Set PYO3_PYTHON and PYTHONPATH to ensure pyo3 uses the venv's Python
export PYO3_PYTHON="$WORKDIR/.venv/bin/python"
export PYTHONPATH="$WORKDIR/.venv/lib/python3.12/site-packages"

# Find and export the Python shared library path for runtime linking
# uv stores Python in ~/.local/share/uv/python/ - we need to find the actual installation
UV_PYTHON_DIR=$(find "$HOME/.local/share/uv/python" -name "libpython3.12.so.1.0" -printf '%h\n' 2>/dev/null | head -1)
if [ -z "$UV_PYTHON_DIR" ]; then
    # Fallback: search in common locations
    UV_PYTHON_DIR=$(find /tmp -name "libpython3.12.so.1.0" -printf '%h\n' 2>/dev/null | head -1)
fi
export LD_LIBRARY_PATH="${UV_PYTHON_DIR:-}:${LD_LIBRARY_PATH:-}"

# Install dependencies using uv
"$UV_BIN" pip install -e ".[dev]"

# Build Python bindings (use --uv since we're using a uv-managed venv)
(cd crates/cli-python && maturin develop --uv)

# Run cargo tests (same as GitHub Action)
cargo test --no-fail-fast --manifest-path ./crates/cli/Cargo.toml
cargo test --no-fail-fast --all --all-features --exclude sqruff
