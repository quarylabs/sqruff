#!/bin/bash
# Run a native Rust test with immutable Python dependencies and private fixtures.
set -euo pipefail

RUNFILES="$RUNFILES_DIR"
BINARY="$RUNFILES/$SQRUFF_NATIVE_TEST_BINARY"
PYTHON="$RUNFILES/$PYTHON_RUNTIME"
WORK="$TEST_TMPDIR/work"
mkdir -p "$WORK" "$TEST_TMPDIR/home" "$TEST_TMPDIR/cache" "$TEST_TMPDIR/tmp"
export HOME="$TEST_TMPDIR/home"
export XDG_CACHE_HOME="$TEST_TMPDIR/cache"
export TMPDIR="$TEST_TMPDIR/tmp"
export PYTHONPYCACHEPREFIX="$TEST_TMPDIR/pycache"
export PYTHONNOUSERSITE=1
export SQRUFF_REQUIRE_PYTHON=1
unset SQRUFF_SKIP_UNSUPPORTED_TEMPLATERS
export PYTHONHOME="$PYTHON"
SITE_PACKAGES=("$PYTHON"/lib/python*/site-packages)
export PYTHONPATH="$WORK/python:${SITE_PACKAGES[0]}"
export LD_LIBRARY_PATH="$PYTHON/lib:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$PYTHON/lib:${DYLD_LIBRARY_PATH:-}"

# Only declared fixture trees are present in this test's runfiles. Copying
# dereferences symlinks for directory walkers and isolates dbt/fix writes.
for crate in "$RUNFILES/_main/crates/"*; do
    for kind in test tests; do
        if [[ -d "$crate/$kind" ]]; then
            mkdir -p "$WORK/crates/$(basename "$crate")"
            cp -rL "$crate/$kind" "$WORK/crates/$(basename "$crate")/"
        fi
    done
done
cp -rL "$RUNFILES/_main/crates/cli-python/python" "$WORK/python"
if [[ -n "${SQRUFF_EXTENSION:-}" ]]; then
    cp -L "$RUNFILES/$SQRUFF_EXTENSION" "$WORK/python/sqruff/_lib_name.so"
fi

# Preserve the actual Python entry point instead of substituting the native CLI.
mkdir -p "$WORK/bin" "$WORK/$TEST_CRATE"
cat > "$WORK/bin/sqruff" <<'LAUNCHER'
#!/bin/bash
set -euo pipefail
exec "$PYTHONHOME/bin/python3" -c 'from sqruff.main import main; main()' "$@"
LAUNCHER
chmod +x "$WORK/bin/sqruff"
export SQRUFF_TEST_MANIFEST_DIR="$WORK/$TEST_CRATE"
export SQRUFF_PYTHON_BIN="$WORK/bin/sqruff"

# Fail on missing Python support before a fixture harness can silently skip it.
"$PYTHON/bin/python3" -c 'import sqruff.templaters.jinja_templater, sqruff.templaters.dbt_templater'
if [[ -n "${SQRUFF_EXTENSION:-}" ]]; then
    "$PYTHON/bin/python3" -c 'from sqruff._lib_name import run_cli; assert callable(run_cli)'
fi
cd "$WORK/$TEST_CRATE"
exec "$BINARY" "$@"
