#!/bin/bash
# Check for unused Rust dependencies using cargo-machete.
# Uses --with-metadata for accurate detection (text search has false positives with symlinked files).
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"
MACHETE_BIN="$RUNFILES_DIR/$MACHETE"

# Create a temp bin directory with symlinks so cargo can find cargo-machete as a subcommand
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
ln -s "$MACHETE_BIN" "$BINDIR/cargo-machete"
export PATH="$BINDIR:$PATH"

cargo machete --with-metadata
