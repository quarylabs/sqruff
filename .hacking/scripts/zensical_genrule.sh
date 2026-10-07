#!/bin/bash
# Build documentation with the cached Python environment.
# Usage: zensical_genrule.sh <output_dir> <python_dir>
set -euo pipefail

EXECROOT="$PWD"
absolute_path() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$EXECROOT/$1" ;;
    esac
}
OUTPUT_DIR="$(absolute_path "$1")"
PYTHON_DIR="$(absolute_path "$2")"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cp -L zensical.toml "$WORKDIR/"
cp -rL docs "$WORKDIR/"
# Zensical's asset discovery skips symlinks. Bazel sandbox inputs are symlinked,
# so materialize the Python package (including the theme) before building.
cp -rL "$PYTHON_DIR" "$WORKDIR/python"
PYTHON_DIR="$WORKDIR/python"
cd "$WORKDIR"

# Invoke the module directly: installed entry-point scripts contain the
# temporary installation prefix, while the Python runtime is relocatable.
export PYTHONHOME="$PYTHON_DIR"
"$PYTHON_DIR/bin/python3" -m zensical build
mkdir -p "$OUTPUT_DIR"
cp -r site/. "$OUTPUT_DIR/"
