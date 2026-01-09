#!/bin/bash
# Run pytest using uv with the specified Python version.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
UV_BIN="$RUNFILES_DIR/$UV"

# Set uv directories to temp locations to avoid read-only filesystem errors in sandbox
TMPDIR="$(mktemp -d)"
export UV_CACHE_DIR="$TMPDIR/cache"
export UV_PYTHON_INSTALL_DIR="$TMPDIR/python"

# Sync dependencies without installing the project itself
echo "Syncing dependencies for Python $PYTHON_VERSION..."
"$UV_BIN" sync --python "$PYTHON_VERSION" --extra dev --no-install-project

# Run pytest using the synced venv
echo "Running pytest..."
"$UV_BIN" run --no-sync pytest

echo "All Python tests passed!"
