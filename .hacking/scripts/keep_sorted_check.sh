#!/bin/bash
# Check that every keep-sorted block in the repository is alphabetically sorted.
# Blocks are opted in by wrapping lines in start/end keep-sorted comments; see
# https://github.com/google/keep-sorted.
set -eo pipefail

cd "$RUNFILES_DIR/_main"
KEEP_SORTED_BIN="$RUNFILES_DIR/$KEEP_SORTED"

# Assembled from two pieces so that this script does not itself look like the
# start of a keep-sorted block when it shows up in the runfiles tree.
MARKER="keep-sorted"
PATTERN="$MARKER start"

# Discover the files that opted in rather than listing them here, so a new
# block only needs its file to be part of the `data` of the Bazel target.
# -R rather than -r: the runfiles tree is made of symlinks to the sources.
mapfile -t FILES < <(grep -Rl --binary-files=without-match "$PATTERN" . | sort)

if [ ${#FILES[@]} -eq 0 ]; then
    echo "ERROR: no keep-sorted blocks found - is the file list still correct?"
    exit 1
fi

echo "Checking ${#FILES[@]} file(s) for unsorted keep-sorted blocks..."

if ! "$KEEP_SORTED_BIN" --mode=lint "${FILES[@]}"; then
    echo ""
    echo "Run 'bazel run //:keep_sorted_fix' to sort these blocks."
    exit 1
fi

echo "All keep-sorted blocks are sorted!"
