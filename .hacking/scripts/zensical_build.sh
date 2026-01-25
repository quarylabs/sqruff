#!/bin/bash
# Build documentation using zensical.
set -eo pipefail

UV_BIN="$RUNFILES_DIR/$UV"

# Create a temp directory for uv cache and build
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -L "$RUNFILES_DIR/_main/pyproject.toml" "$WORKDIR/"
cp -L "$RUNFILES_DIR/_main/uv.lock" "$WORKDIR/"
cp -L "$RUNFILES_DIR/_main/zensical.toml" "$WORKDIR/"
cp -rL "$RUNFILES_DIR/_main/docs" "$WORKDIR/"

cd "$WORKDIR"

# Set uv directories to temp locations to avoid read-only filesystem errors in sandbox
export UV_CACHE_DIR="$WORKDIR/.uv-cache"
export UV_PYTHON_INSTALL_DIR="$WORKDIR/.uv-python"

# Sync dependencies
echo "Syncing dependencies..."
"$UV_BIN" sync --extra docs --no-install-project

# Build docs using zensical
echo "Building documentation..."
"$UV_BIN" run --no-sync zensical build

echo "Documentation built successfully!"
echo "Output: $WORKDIR/site"

# List the output
ls -la "$WORKDIR/site" || echo "No site directory created"
