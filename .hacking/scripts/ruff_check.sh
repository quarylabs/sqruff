#!/bin/bash
# Check Python code with ruff (format and lint).
set -eo pipefail

cd "$RUNFILES_DIR/_main"
RUFF_BIN="$RUNFILES_DIR/$RUFF"

# Run ruff format check
echo "Checking Python formatting with ruff..."
"$RUFF_BIN" format --check .

# Run ruff linter
echo "Running ruff linter..."
"$RUFF_BIN" check .

echo "All ruff checks passed!"
