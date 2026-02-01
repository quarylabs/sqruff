#!/bin/bash
# Run cargo clippy with all features and treat warnings as errors.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"

# Create a temp bin directory with cargo symlink
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
export PATH="$BINDIR:$PATH"

# Run cargo clippy
cargo clippy --all --all-features -- -D warnings
