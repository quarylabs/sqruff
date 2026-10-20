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
export SQLMESH_HOME="$STATE_DIR/sqlmesh"

# Copy project files to temp directory (use -L to follow symlinks from runfiles)
cp -rL "$RUNFILES_DIR/_main/crates" "$WORKDIR/"

if [[ -n "${SQRUFF_EXTENSION:-}" ]]; then
    cp -L "$RUNFILES_DIR/$SQRUFF_EXTENSION" "$WORKDIR/crates/cli-python/python/sqruff/_sqruff.so"
fi

cd "$WORKDIR"

# Export PROJECT_ROOT so tests can find files relative to the temp directory
export PROJECT_ROOT="$WORKDIR"

PYTHON_BIN="$VENV_DIR/bin/python3"

# Native extensions must use the interpreter that supplies their libpython.
# A standalone uv interpreter can load a second Python runtime on macOS.
if [[ -n "${SQRUFF_EXTENSION:-}" ]]; then
    PYTHON_BIN="$RUNFILES_DIR/_main/$PYTHON_INTERPRETER"
    PYTHON_HOME="$(dirname "$(dirname "$PYTHON_BIN")")"
    export PYTHONHOME="$PYTHON_HOME"
    export LD_LIBRARY_PATH="$PYTHON_HOME/lib:${LD_LIBRARY_PATH:-}"
    export DYLD_LIBRARY_PATH="$PYTHON_HOME/lib:${DYLD_LIBRARY_PATH:-}"
fi

# The hermetic venv is a copied python-build-standalone install. After Bazel
# relocates it into the runfiles tree, the interpreter can fail to resolve its
# own site-packages, surfacing as "No module named pytest". Point PYTHONPATH at
# the venv's site-packages explicitly so the installed deps are importable.
SITE_PACKAGES="$(echo "$VENV_DIR"/lib/python*/site-packages)"
export PYTHONPATH="$WORKDIR/crates/cli-python/python:$SITE_PACKAGES${PYTHONPATH:+:$PYTHONPATH}"

# The dedicated SQLMesh target must exercise the optional dependency rather
# than silently skipping its project-rendering tests.
if [[ "${SQRUFF_REQUIRE_SQLMESH:-0}" == "1" ]]; then
    "$PYTHON_BIN" -c 'import sqlmesh'
fi

# Run pytest using the cached venv's Python
echo "Running pytest..."
"$PYTHON_BIN" -m pytest "$@"

echo "All Python tests passed!"
