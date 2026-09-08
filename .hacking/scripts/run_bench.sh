#!/bin/bash
# Resolve declared Bazel inputs before invoking the native benchmark binary.
set -euo pipefail

RUNFILES_DIR="${RUNFILES_DIR:-$0.runfiles}"

BENCH="$RUNFILES_DIR/$1"
SQRUFF="$RUNFILES_DIR/$2"
FIXTURE="$RUNFILES_DIR/$3"
shift 3

# Directory traversal skips symlinked runfiles. Materialize fixtures before
# starting the timed process, outside Bazel's ignored output directories.
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cp -rL "$FIXTURE" "$WORKDIR/$(basename "$FIXTURE")"
"$BENCH" "$SQRUFF" "$WORKDIR/$(basename "$FIXTURE")" "$@"
