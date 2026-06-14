#!/usr/bin/env bash
# Verifies that an update to .sqlfluff-sha moves the SHA exactly one commit
# forward along sqlfluff/sqlfluff's main branch (first-parent only, matching
# the "next-sqlfluff" porting workflow).
#
# Usage: check_sqlfluff_sha_advance.sh <old-sha> <new-sha>
#
# Exits 0 when <new-sha> is the immediate next first-parent commit after
# <old-sha> on main, and non-zero otherwise (e.g. it skips commits, goes
# backwards, or either SHA is unknown to sqlfluff).

set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <old-sha> <new-sha>" >&2
    exit 2
fi

OLD=$(echo "$1" | tr -d '[:space:]')
NEW=$(echo "$2" | tr -d '[:space:]')

for var in OLD NEW; do
    value="${!var}"
    if ! echo "$value" | grep -qE '^[0-9a-f]{40}$'; then
        echo "ERROR: ${var} is not a valid 40-character hex SHA: '${value}'" >&2
        exit 1
    fi
done

if [ "$OLD" = "$NEW" ]; then
    echo "OK: .sqlfluff-sha is unchanged (${NEW}); nothing to validate"
    exit 0
fi

# Obtain a clone of sqlfluff with full history. Reuse an existing checkout at
# ./sqlfluff if present (e.g. local runs), otherwise clone into a temp dir.
if [ -d "sqlfluff/.git" ]; then
    REPO="sqlfluff"
    git -C "$REPO" fetch --quiet origin main
else
    REPO=$(mktemp -d)
    trap 'rm -rf "$REPO"' EXIT
    git clone --quiet https://github.com/sqlfluff/sqlfluff.git "$REPO"
fi

# Determine main's ref (origin/main when fetched, main otherwise).
if git -C "$REPO" rev-parse --verify --quiet origin/main >/dev/null; then
    MAIN="origin/main"
else
    MAIN="main"
fi

if ! git -C "$REPO" cat-file -e "${OLD}^{commit}" 2>/dev/null; then
    echo "ERROR: old SHA ${OLD} does not exist in sqlfluff/sqlfluff" >&2
    exit 1
fi

if ! git -C "$REPO" cat-file -e "${NEW}^{commit}" 2>/dev/null; then
    echo "ERROR: new SHA ${NEW} does not exist in sqlfluff/sqlfluff" >&2
    exit 1
fi

# The next commit after OLD on main, following first parents only (so merge
# commits are followed but side-branch commits are skipped). This mirrors
# `git log --reverse --first-parent <SHA>..main | head -n 1` from the
# next-sqlfluff workflow.
#
# pipefail is disabled for this pipeline only: `head -n 1` closes the pipe
# early, sending SIGPIPE to `git log` (exit 141), which is expected here.
set +o pipefail
EXPECTED=$(git -C "$REPO" log --reverse --first-parent --format="%H" "${OLD}..${MAIN}" | head -n 1)
set -o pipefail

if [ -z "$EXPECTED" ]; then
    echo "ERROR: old SHA ${OLD} is already at (or ahead of) main; there is no next commit to advance to." >&2
    echo "  Is ${OLD} on main's first-parent history?" >&2
    exit 1
fi

if [ "$NEW" = "$EXPECTED" ]; then
    echo "OK: .sqlfluff-sha advances exactly one commit on main:"
    echo "  ${OLD} -> ${NEW}"
    exit 0
fi

echo "ERROR: .sqlfluff-sha must advance exactly one commit forward on sqlfluff main." >&2
echo "  old:      ${OLD}" >&2
echo "  new:      ${NEW}" >&2
echo "  expected: ${EXPECTED}" >&2
echo "" >&2

# Provide a helpful diagnosis of why it failed.
if git -C "$REPO" merge-base --is-ancestor "$NEW" "$OLD" 2>/dev/null; then
    echo "  The new SHA is an ancestor of the old SHA (the update goes backwards)." >&2
elif git -C "$REPO" merge-base --is-ancestor "$OLD" "$NEW" 2>/dev/null; then
    COUNT=$(git -C "$REPO" rev-list --count --first-parent "${OLD}..${NEW}" 2>/dev/null || echo "?")
    echo "  The new SHA is ${COUNT} first-parent commits ahead of the old SHA; only one step is allowed." >&2
    echo "  Set .sqlfluff-sha to ${EXPECTED} to advance a single commit." >&2
else
    echo "  The new SHA is not on main's first-parent history after the old SHA." >&2
    echo "  Set .sqlfluff-sha to ${EXPECTED} to advance a single commit." >&2
fi

exit 1
