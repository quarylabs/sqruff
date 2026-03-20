#!/bin/bash
# Run pytest using a pre-built Python venv from Bazel cache.
set -eo pipefail

VENV_DIR="$RUNFILES_DIR/$PYTHON_VENV"

# Create a temp directory and copy everything there so paths are consistent
# This is needed because dbt parses the project and stores paths in its manifest,
# and those paths need to match when we later search for files.
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -rL "$RUNFILES_DIR/_main/crates" "$WORKDIR/"

cd "$WORKDIR"

# Export PROJECT_ROOT so tests can find files relative to the temp directory
export PROJECT_ROOT="$WORKDIR"

PYTHON_BIN="$VENV_DIR/bin/python3"

# Run pytest using the cached venv's Python
echo "Running pytest..."
"$PYTHON_BIN" -m pytest

echo "All Python tests passed!"
