#!/usr/bin/env bash
# Verify that uv.lock is up to date with pyproject.toml.
#
# Runs `uv lock --locked`, which asserts the lockfile would remain unchanged and
# fails when uv.lock no longer matches pyproject.toml (for example after a
# dependency bump that forgot to regenerate the lockfile).
#
# The project files are copied into a writable temp directory because Bazel
# runfiles are read-only symlinks and uv wants a writable tree.
set -euo pipefail

UV_BIN="$RUNFILES_DIR/$UV"

# Create a temp directory for uv cache and a writable workspace.
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# Copy only the files that determine the resolved lockfile (use -L to follow
# symlinks from runfiles).
cp -L "$RUNFILES_DIR/_main/pyproject.toml" "$WORKDIR/"
cp -L "$RUNFILES_DIR/_main/uv.lock" "$WORKDIR/"

cd "$WORKDIR"

# Set uv directories to temp locations to avoid read-only filesystem errors in
# the sandbox.
export UV_CACHE_DIR="$WORKDIR/.uv-cache"
export UV_PYTHON_INSTALL_DIR="$WORKDIR/.uv-python"

if ! "$UV_BIN" lock --locked; then
    echo ""
    echo "ERROR: uv.lock is out of date with pyproject.toml."
    echo "Run 'uv lock' and commit the updated uv.lock."
    exit 1
fi

echo "OK: uv.lock is up to date"
