#!/bin/bash
# Run cargo build --release with all features to verify release builds work.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"

# Create a temp bin directory with cargo symlink
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
export PATH="$BINDIR:$PATH"

# Run cargo build release
cargo build --locked --release --all-features
