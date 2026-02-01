#!/bin/bash
# Run cargo fmt check to verify code formatting.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"

# Create a temp bin directory with cargo symlink
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
export PATH="$BINDIR:$PATH"

# Run cargo fmt check
cargo fmt --all -- --check
