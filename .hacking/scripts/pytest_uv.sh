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

# Dependencies were installed into this interpreter's sysconfig purelib via
# `uv pip install --prefix`. Some relocated standalone Python builds (notably
# on Linux) do not auto-add that directory to sys.path, which manifests as
# "No module named pytest". Ask the interpreter where purelib lives and put it
# on PYTHONPATH so imports resolve consistently across platforms.
SITE_PACKAGES="$("$PYTHON_BIN" -c 'import sysconfig; print(sysconfig.get_path("purelib"))')"
export PYTHONPATH="$SITE_PACKAGES${PYTHONPATH:+:$PYTHONPATH}"

# Run pytest using the cached venv's Python
echo "Running pytest..."
"$PYTHON_BIN" -m pytest

echo "All Python tests passed!"
