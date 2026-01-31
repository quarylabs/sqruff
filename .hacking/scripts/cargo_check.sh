#!/bin/bash
# Run cargo check with all features, tests, and benches.
# This replicates the "Rust Check" GitHub action job.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"

# Create a temp bin directory with cargo symlink
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
export PATH="$BINDIR:$PATH"

# Run cargo check with all features, tests, and benches
cargo check --all --all-features --tests --benches --locked
