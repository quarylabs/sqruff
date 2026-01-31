#!/bin/bash
# Fix Rust formatting with rustfmt.
# Run with: bazel run //:rustfmt_fix
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"

echo "Fixing Rust formatting with rustfmt..."
cargo fmt --all

echo "Rustfmt fixes applied"
