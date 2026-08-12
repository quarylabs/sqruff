#!/bin/bash
# Sort every keep-sorted block in the repository in place.
set -eo pipefail

KEEP_SORTED_BIN="$0.runfiles/$KEEP_SORTED"
cd "$BUILD_WORKSPACE_DIRECTORY"

# Assembled from two pieces so that this script does not itself look like the
# start of a keep-sorted block.
MARKER="keep-sorted"
PATTERN="$MARKER start"

# //:keep_sorted_check lints the srcs of the Rust targets via the rules_lint
# aspect; here we sweep the whole workspace instead, minus the generated and
# vendored trees Bazel ignores. Fixing a superset of what is checked is safe.
mapfile -t FILES < <(grep -rl --binary-files=without-match "$PATTERN" . \
    --exclude-dir=.git \
    --exclude-dir=dist \
    --exclude-dir=node_modules \
    --exclude-dir=target \
    --exclude-dir=.venv | sort)

if [ ${#FILES[@]} -eq 0 ]; then
    echo "No keep-sorted blocks found."
    exit 0
fi

echo "Sorting keep-sorted blocks in ${#FILES[@]} file(s)..."
"$KEEP_SORTED_BIN" --mode=fix "${FILES[@]}"

echo "All keep-sorted blocks sorted!"
