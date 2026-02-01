#!/bin/bash
# Run cargo hack check --each-feature to verify all feature combinations compile.
# This ensures individual features don't break when enabled in isolation.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"
CARGO_HACK_BIN="$RUNFILES_DIR/$CARGO_HACK"

# Create a temp bin directory with symlinks so cargo can find cargo-hack as a subcommand
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
ln -s "$CARGO_HACK_BIN" "$BINDIR/cargo-hack"
export PATH="$BINDIR:$PATH"

# Run cargo hack check --each-feature
cargo hack check --each-feature --exclude-features=codegen-docs
