#!/bin/bash
# Build documentation using zensical and output to specified directory.
# Usage: zensical_genrule.sh <output_dir>
set -eo pipefail

OUTPUT_DIR_ARG="$1"

# Save the original working directory (Bazel execroot)
EXECROOT="$(pwd)"

# Convert the output path to absolute if it's relative
if [[ "$OUTPUT_DIR_ARG" == /* ]]; then
    OUTPUT_DIR="$OUTPUT_DIR_ARG"
else
    OUTPUT_DIR="$EXECROOT/$OUTPUT_DIR_ARG"
fi

# Find runfiles directory - it's alongside the script with .runfiles suffix
# When run via run_binary, RUNFILES_DIR may not be set, so we find it ourselves
if [[ -z "$RUNFILES_DIR" ]]; then
    # Get the directory where this script is located
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    RUNFILES_DIR="${SCRIPT_DIR}/zensical_genrule_tool.runfiles"
fi

# Find UV binary - the path from @uv//:uv is +tools+uv/uv
UV_BIN="$RUNFILES_DIR/+tools+uv/uv"

if [[ ! -e "$UV_BIN" ]]; then
    echo "ERROR: Could not find uv binary at $UV_BIN"
    ls -la "$RUNFILES_DIR" || true
    exit 1
fi

# Create a temp directory for uv cache and build
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# The tool's data dependencies are in runfiles
# Copy project files to temp directory (use -L to follow symlinks)
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

# Copy output to the specified directory (using absolute path)
cp -r "$WORKDIR/site/"* "$OUTPUT_DIR/"

echo "Documentation built successfully!"
