#!/bin/bash
# Fix Python code with ruff (format and lint).
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"
RUFF_BIN="$0.runfiles/$RUFF"

# Run ruff format
echo "Formatting Python with ruff..."
"$RUFF_BIN" format .

# Run ruff linter with --fix
echo "Running ruff linter with --fix..."
"$RUFF_BIN" check --fix .

echo "All ruff fixes applied!"
