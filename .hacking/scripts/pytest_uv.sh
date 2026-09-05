#!/bin/bash
# Run pytest using a pre-built Python venv from Bazel cache.
set -eo pipefail

VENV_DIR="$RUNFILES_DIR/$PYTHON_VENV"

# Keep every writable location inside Bazel's per-test temporary directory so
# Python versions can run concurrently without sharing state through $HOME,
# caches, bytecode, temporary files, or coverage data.
STATE_DIR="$TEST_TMPDIR/state"
WORKDIR="$TEST_TMPDIR/work"
mkdir -p \
    "$STATE_DIR/home" \
    "$STATE_DIR/xdg-cache" \
    "$STATE_DIR/pycache" \
    "$STATE_DIR/tmp" \
    "$WORKDIR"

export HOME="$STATE_DIR/home"
export XDG_CACHE_HOME="$STATE_DIR/xdg-cache"
export PYTHONPYCACHEPREFIX="$STATE_DIR/pycache"
export TMPDIR="$STATE_DIR/tmp"
export COVERAGE_FILE="$STATE_DIR/.coverage"
export PYTHONNOUSERSITE=1

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -rL "$RUNFILES_DIR/_main/crates" "$WORKDIR/"

cd "$WORKDIR"

# Export PROJECT_ROOT so tests can find files relative to the temp directory
export PROJECT_ROOT="$WORKDIR"

PYTHON_BIN="$VENV_DIR/bin/python3"

# The hermetic venv is a copied python-build-standalone install. After Bazel
# relocates it into the runfiles tree, the interpreter can fail to resolve its
# own site-packages, surfacing as "No module named pytest". Point PYTHONPATH at
# the venv's site-packages explicitly so the installed deps are importable.
SITE_PACKAGES="$(echo "$VENV_DIR"/lib/python*/site-packages)"
export PYTHONPATH="$SITE_PACKAGES${PYTHONPATH:+:$PYTHONPATH}"

# Run pytest using the cached venv's Python
echo "Running pytest..."
"$PYTHON_BIN" -m pytest

echo "All Python tests passed!"
