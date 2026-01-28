#!/bin/bash
# Regenerate docs using the codegen binary.
# Run with: bazel run //:codegen_docs_fix
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"

echo "Regenerating docs..."
cargo run --bin sqruff -F codegen-docs

echo "Docs regenerated successfully"
