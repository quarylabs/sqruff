#!/bin/bash
# Build WASM package for the playground using wasm-pack.
# This script should be run before building the playground.
# Usage: ./.hacking/scripts/build_wasm.sh
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$ROOT_DIR"

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack is not installed. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build the WASM package with size optimization
echo "Building WASM package..."
RUSTFLAGS="-C opt-level=z" wasm-pack build \
    crates/lib-wasm \
    --target web \
    --out-dir ../../playground/src/pkg

echo "WASM package built successfully at playground/src/pkg/"
