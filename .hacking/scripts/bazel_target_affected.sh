#!/usr/bin/env bash
# Reports whether a Bazel target is affected between two Git revisions.
#
# Usage: bazel_target_affected.sh <base-sha> <head-sha> <target> [pathspec ...]
#
# Prints "true" or "false" to stdout. Tool output is sent to stderr so callers
# can safely capture the result. BAZEL_DIFF and BAZEL may override the binaries
# used for target hashing and Bazel queries. A change matching any optional Git
# pathspec is treated as affecting the target without consulting Bazel. This is
# useful for build inputs which the target graph does not model.

set -euo pipefail

if [ "$#" -lt 3 ]; then
    echo "Usage: $0 <base-sha> <head-sha> <target> [pathspec ...]" >&2
    exit 2
fi

BASE_SHA=$1
HEAD_SHA=$2
TARGET=$3
shift 3
ALWAYS_AFFECTING_PATHS=("$@")
BAZEL_DIFF=${BAZEL_DIFF:-bazel-diff}
BAZEL=${BAZEL:-bazelisk}

for sha in "$BASE_SHA" "$HEAD_SHA"; do
    if ! git cat-file -e "${sha}^{commit}" 2>/dev/null; then
        echo "ERROR: ${sha} is not an available Git commit" >&2
        exit 1
    fi
done

if [ "${#ALWAYS_AFFECTING_PATHS[@]}" -gt 0 ]; then
    ALWAYS_AFFECTING_CHANGES=$(git diff --name-only --no-renames \
        "$BASE_SHA" "$HEAD_SHA" -- "${ALWAYS_AFFECTING_PATHS[@]}")
    if [ -n "$ALWAYS_AFFECTING_CHANGES" ]; then
        echo "Changed build inputs not modeled by Bazel:" >&2
        printf '%s\n' "$ALWAYS_AFFECTING_CHANGES" >&2
        echo true
        exit 0
    fi
fi

if ! BAZEL_DIFF=$(command -v "$BAZEL_DIFF"); then
    echo "ERROR: bazel-diff executable not found" >&2
    exit 1
fi

if ! BAZEL=$(command -v "$BAZEL"); then
    echo "ERROR: Bazel executable not found" >&2
    exit 1
fi

TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/sqruff-bazel-affected.XXXXXX")
trap 'rm -rf "$TEMP_DIR"' EXIT

TEMP_WORKSPACE="${TEMP_DIR}/workspace"
MODIFIED_FILES="${TEMP_DIR}/modified-files.txt"
BASE_HASHES="${TEMP_DIR}/base-hashes.json"
HEAD_HASHES="${TEMP_DIR}/head-hashes.json"
IMPACTED_TARGETS="${TEMP_DIR}/impacted-targets.txt"

# Disable rename collapsing so both sides of a rename are included. The
# modified-file list must be a superset for bazel-diff's content-hash shortcut.
git diff --name-only --no-renames "$BASE_SHA" "$HEAD_SHA" > "$MODIFIED_FILES"

# Hash both revisions in one temporary checkout. Reusing its Bazel output base
# lets the second query incrementally update the first graph instead of starting
# a second Bazel server and resolving every external repository again.
git clone --quiet --shared --no-checkout "$(git rev-parse --show-toplevel)" "$TEMP_WORKSPACE"
git -C "$TEMP_WORKSPACE" reset --hard --quiet "$BASE_SHA"
"$BAZEL_DIFF" generate-hashes \
    --workspacePath "$TEMP_WORKSPACE" \
    --bazelPath "$BAZEL" \
    --excludeExternalTargets \
    --modified-filepaths "$MODIFIED_FILES" \
    "$BASE_HASHES" >&2

git -C "$TEMP_WORKSPACE" reset --hard --quiet "$HEAD_SHA"
"$BAZEL_DIFF" generate-hashes \
    --workspacePath "$TEMP_WORKSPACE" \
    --bazelPath "$BAZEL" \
    --excludeExternalTargets \
    --modified-filepaths "$MODIFIED_FILES" \
    "$HEAD_HASHES" >&2

"$BAZEL_DIFF" get-impacted-targets \
    --workspacePath "$TEMP_WORKSPACE" \
    --bazelPath "$BAZEL" \
    --startingHashes "$BASE_HASHES" \
    --finalHashes "$HEAD_HASHES" \
    --output "$IMPACTED_TARGETS" >&2

echo "Impacted Bazel targets:" >&2
sed -n '1,200p' "$IMPACTED_TARGETS" >&2

if grep -Fxq -- "$TARGET" "$IMPACTED_TARGETS"; then
    echo true
else
    echo false
fi
