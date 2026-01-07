#!/bin/bash
set -eo pipefail

# Find workspace root - works for both bazel run and bazel test (with local=True)
if [[ -n "$BUILD_WORKSPACE_DIRECTORY" ]]; then
    cd "$BUILD_WORKSPACE_DIRECTORY"
elif [[ -f "Cargo.toml" ]]; then
    : # Already in workspace root
else
    # Navigate from script location
    cd "$(dirname "$0")/../.."
fi

cargo hack check --each-feature --exclude-features=codegen-docs
