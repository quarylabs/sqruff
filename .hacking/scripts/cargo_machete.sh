#!/bin/bash
# Check for unused Rust dependencies using cargo-machete.
# Copies files to a temp directory first because cargo-machete's file walker
# doesn't follow symlinks, and bazel runfiles use symlinks.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"
MACHETE_BIN="$RUNFILES_DIR/$MACHETE"

# Create a temp bin directory with symlinks so cargo can find cargo-machete as a subcommand
BINDIR=$(mktemp -d)
ln -s "$CARGO_BIN" "$BINDIR/cargo"
ln -s "$MACHETE_BIN" "$BINDIR/cargo-machete"
export PATH="$BINDIR:$PATH"

# Copy files to a temp directory to resolve symlinks
# cargo-machete uses the `ignore` crate which doesn't follow symlinks
WORKDIR=$(mktemp -d)
cp -rL . "$WORKDIR/"
cd "$WORKDIR"

# Run cargo machete
cargo machete
