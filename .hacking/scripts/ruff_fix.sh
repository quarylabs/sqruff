#!/bin/bash
# Fix Python code with ruff (format and lint).
# Run with: bazel run //:ruff_fix
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"

echo "Fixing Python formatting with ruff..."
uv run ruff format .

echo "Fixing Python lint issues with ruff..."
uv run ruff check --fix . || true

echo "Ruff fixes applied"
