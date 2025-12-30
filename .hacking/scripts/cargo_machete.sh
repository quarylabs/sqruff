#!/bin/bash
set -eo pipefail

# In Bazel sandbox, RUNFILES_DIR points to where data files are
if [[ -n "$RUNFILES_DIR" ]]; then
    cd "$RUNFILES_DIR/_main"

    # Resolve cargo path from runfiles
    if [[ -n "$CARGO" ]]; then
        CARGO_BIN="$RUNFILES_DIR/$CARGO"
    else
        CARGO_BIN="cargo"
    fi
else
    CARGO_BIN="cargo"
fi

# cargo-machete must be installed: cargo install cargo-machete
"$CARGO_BIN" machete
