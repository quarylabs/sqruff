#!/bin/bash
# Fix formatting with prettier.
# Run with: bazel run //:prettier_fix
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"

# Use pnpm from PATH (assumes pnpm is installed)
pnpm prettier --write --config .prettierrc.json editors playground "*.md" .github

echo "Prettier fixes applied"
