#!/bin/bash
# Build and optionally serve documentation using zensical.
set -eo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"

# Sync dependencies
echo "Syncing dependencies..."
uv sync --extra docs --no-install-project

# Check if --serve flag is passed
if [[ "$1" == "--serve" ]]; then
    echo "Building and serving documentation..."
    uv run --no-sync zensical serve
else
    echo "Building documentation..."
    uv run --no-sync zensical build
    echo ""
    echo "Documentation built successfully!"
    echo "Output: $BUILD_WORKSPACE_DIRECTORY/site"
    echo ""
    echo "To serve locally, run: bazel run //:zensical_serve -- --serve"
    echo "Or open: file://$BUILD_WORKSPACE_DIRECTORY/site/index.html"
fi
