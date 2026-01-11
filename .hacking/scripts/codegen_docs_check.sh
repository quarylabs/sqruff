#!/bin/bash
# Checks that generated documentation files are up-to-date.
# Runs codegen-docs and compares output with committed files.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
CARGO_BIN="$RUNFILES_DIR/$CARGO"

# Create temp directories for cargo build and doc backups
BACKUP_DIR=$(mktemp -d)
TARGET_DIR=$(mktemp -d)
trap 'rm -rf "$BACKUP_DIR" "$TARGET_DIR"' EXIT

# Use temp directory for cargo builds (runfiles may be read-only)
export CARGO_TARGET_DIR="$TARGET_DIR"

# Backup current docs
cp docs/cli.md "$BACKUP_DIR/cli.md"
cp docs/rules.md "$BACKUP_DIR/rules.md"
cp docs/templaters.md "$BACKUP_DIR/templaters.md"

# Run codegen (suppress GitHub Actions autocommit behavior)
env GITHUB_ACTIONS=false "$CARGO_BIN" run --bin sqruff -F codegen-docs

# Compare generated files with originals
exit_code=0

for file in cli.md rules.md templaters.md; do
    if ! diff -q "docs/$file" "$BACKUP_DIR/$file" > /dev/null 2>&1; then
        echo "ERROR: docs/$file is out of date"
        echo "Please run: cargo run --bin sqruff -F codegen-docs"
        echo ""
        echo "Diff:"
        diff "docs/$file" "$BACKUP_DIR/$file" || true
        exit_code=1
    else
        echo "OK: docs/$file is up-to-date"
    fi
done

exit $exit_code
