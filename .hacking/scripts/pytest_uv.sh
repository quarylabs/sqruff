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

# Dependencies were installed via `uv pip install --prefix` into the standalone
# interpreter's site-packages. Some relocated standalone Python builds (notably
# on Linux) do not auto-add that directory to sys.path, which manifests as
# "No module named pytest". Locate the installed packages directly and force
# them onto PYTHONPATH so imports resolve consistently across platforms.
for sp in $(find "$VENV_DIR" -type d \( -name site-packages -o -name dist-packages \) 2>/dev/null); do
    PYTHONPATH="$sp${PYTHONPATH:+:$PYTHONPATH}"
done
export PYTHONPATH

echo "Diagnostics: PYTHONPATH=$PYTHONPATH"
if ! "$PYTHON_BIN" -c 'import pytest' 2>/dev/null; then
    echo "ERROR: pytest not importable. Dumping venv layout for diagnosis:" >&2
    find "$VENV_DIR" -maxdepth 4 -name 'pytest' -o -name '_pytest' 2>/dev/null | head >&2 || true
    "$PYTHON_BIN" -c 'import sys, sysconfig; print("sys.prefix=", sys.prefix); print("purelib=", sysconfig.get_path("purelib")); print("sys.path=", sys.path)' >&2 || true
fi

# Run pytest using the cached venv's Python
echo "Running pytest..."
"$PYTHON_BIN" -m pytest

echo "All Python tests passed!"
