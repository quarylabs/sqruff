#!/bin/bash
# Install only a built wheel, then exercise the installed extension and CLI.
set -euo pipefail

PYTHON="$RUNFILES_DIR/$PYTHON_RUNTIME/bin/python3"
UV="$RUNFILES_DIR/$UV"
WHEELS="$RUNFILES_DIR/$PYTHON_WHEELS"

unset PYTHONPATH PYTHONHOME
export PYTHONNOUSERSITE=1
export UV_CACHE_DIR="$TEST_TMPDIR/uv-cache"
export UV_PYTHON_DOWNLOADS=never

# Keep the checkout and its Python modules off sys.path. Each matrix test gets
# its own environment, and package installation has no network or source fallback.
cd "$TEST_TMPDIR"
"$UV" venv --python "$PYTHON" --no-project --no-config .venv
"$UV" pip install --python .venv/bin/python --no-config --offline \
    --no-index --no-deps --find-links "$WHEELS" --only-binary=:all: sqruff
.venv/bin/python -I -c 'import sqruff._lib_name'
.venv/bin/sqruff --version
