#!/bin/bash
# Run pytest using uv with the specified Python version.
set -eo pipefail

UV_BIN="$RUNFILES_DIR/$UV"

# Create a temp directory and copy everything there so paths are consistent
# This is needed because dbt parses the project and stores paths in its manifest,
# and those paths need to match when we later search for files.
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -L "$RUNFILES_DIR/_main/pyproject.toml" "$WORKDIR/"
cp -L "$RUNFILES_DIR/_main/uv.lock" "$WORKDIR/"
cp -rL "$RUNFILES_DIR/_main/crates" "$WORKDIR/"

cd "$WORKDIR"

# Set uv directories to temp locations to avoid read-only filesystem errors in sandbox
export UV_CACHE_DIR="$WORKDIR/.uv-cache"
export UV_PYTHON_INSTALL_DIR="$WORKDIR/.uv-python"

# Export PROJECT_ROOT so tests can find files relative to the temp directory
export PROJECT_ROOT="$WORKDIR"

# Sync dependencies without installing the project itself
# Use --extra test (not --extra dev) to avoid downloading large tools like ruff
# that are provided separately by Bazel
echo "Syncing dependencies for Python $PYTHON_VERSION..."
"$UV_BIN" sync --python "$PYTHON_VERSION" --extra test --no-install-project

# Run pytest using the synced venv
echo "Running pytest..."
"$UV_BIN" run --no-sync pytest

echo "All Python tests passed!"
