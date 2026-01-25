#!/bin/bash
# Check that generated docs are up to date.
# Runs the codegen binary and verifies the output matches what's checked in.
set -eo pipefail

cd "$RUNFILES_DIR/_main"

# Get the codegen binary path
CODEGEN_BIN="$RUNFILES_DIR/$CODEGEN"

# Create temp directory for generated output
WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

# Copy the docs directory structure (codegen writes to docs/reference/)
mkdir -p "$WORKDIR/docs/reference"

# Run codegen from the workdir
cd "$WORKDIR"
GITHUB_ACTIONS=false "$CODEGEN_BIN"

# Compare the generated files with the originals from runfiles
FAILED=0

if ! diff -q "$RUNFILES_DIR/_main/docs/reference/cli.md" "$WORKDIR/docs/reference/cli.md" > /dev/null 2>&1; then
    echo "ERROR: docs/reference/cli.md is out of date"
    echo "Run 'cargo run --bin sqruff -F codegen-docs' to update"
    echo ""
    diff "$RUNFILES_DIR/_main/docs/reference/cli.md" "$WORKDIR/docs/reference/cli.md" || true
    FAILED=1
fi

if ! diff -q "$RUNFILES_DIR/_main/docs/reference/rules.md" "$WORKDIR/docs/reference/rules.md" > /dev/null 2>&1; then
    echo "ERROR: docs/reference/rules.md is out of date"
    echo "Run 'cargo run --bin sqruff -F codegen-docs' to update"
    echo ""
    diff "$RUNFILES_DIR/_main/docs/reference/rules.md" "$WORKDIR/docs/reference/rules.md" || true
    FAILED=1
fi

if ! diff -q "$RUNFILES_DIR/_main/docs/reference/templaters.md" "$WORKDIR/docs/reference/templaters.md" > /dev/null 2>&1; then
    echo "ERROR: docs/reference/templaters.md is out of date"
    echo "Run 'cargo run --bin sqruff -F codegen-docs' to update"
    echo ""
    diff "$RUNFILES_DIR/_main/docs/reference/templaters.md" "$WORKDIR/docs/reference/templaters.md" || true
    FAILED=1
fi

if [ $FAILED -eq 1 ]; then
    exit 1
fi

echo "All generated docs are up to date"
