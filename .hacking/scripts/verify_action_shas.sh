#!/usr/bin/env bash
# Verifies that all pinned GitHub Actions SHAs actually exist in their repos.
# This catches cases where a SHA was typo'd, force-pushed away, or references
# a non-existent tag/version.

set -euo pipefail

FAILED=0
CHECKED=0
SEEN_FILE=$(mktemp)
trap 'rm -f "$SEEN_FILE"' EXIT

PIDS=()

wait_for_checks() {
    local pid
    for pid in "${PIDS[@]}"; do
        if ! wait "$pid"; then
            FAILED=1
        fi
    done
    PIDS=()
}

verify_sha() {
    local file="$1" repo="$2" sha="$3" status
    # Treat transport errors as failures too, without stopping other checks.
    if ! status=$(curl -s -o /dev/null -w "%{http_code}" \
        ${GITHUB_TOKEN:+-H "Authorization: token $GITHUB_TOKEN"} \
        "https://api.github.com/repos/${repo}/git/commits/${sha}"); then
        echo "ERROR: ${file}: Could not verify ${repo}@${sha} (request failed)"
        return 1
    fi
    if [ "$status" != "200" ]; then
        echo "ERROR: ${file}: ${repo}@${sha} does not exist (HTTP ${status})"
        return 1
    fi
}

for file in .github/workflows/*.yml; do
    # Extract owner/repo@sha patterns from uses: directives
    # verify_sha uses the file argument only as an error label, never writes it.
    # shellcheck disable=SC2094
    while IFS= read -r line; do
        action=$(echo "$line" | grep -oE '[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+@[0-9a-f]{40}' || true)
        if [ -z "$action" ]; then
            continue
        fi

        repo="${action%@*}"
        sha="${action#*@}"
        key="${repo}@${sha}"

        # Skip if we've already checked this exact ref
        if grep -qxF "$key" "$SEEN_FILE" 2>/dev/null; then
            continue
        fi
        echo "$key" >> "$SEEN_FILE"

        CHECKED=$((CHECKED + 1))
        # Check batches of four refs, keeping API concurrency bounded.
        verify_sha "$file" "$repo" "$sha" &
        PIDS+=("$!")
        if [ "${#PIDS[@]}" -eq 4 ]; then
            wait_for_checks
        fi
    done < "$file"
done

if [ "${#PIDS[@]}" -gt 0 ]; then
    wait_for_checks
fi

if [ "$CHECKED" -eq 0 ]; then
    echo "ERROR: No action SHAs found to verify"
    exit 1
fi

if [ "$FAILED" -eq 1 ]; then
    echo ""
    echo "Some GitHub Actions SHAs could not be resolved."
    echo "Check that the pinned SHAs are correct and the commits still exist."
    exit 1
fi

echo "OK: All ${CHECKED} unique action SHAs verified successfully"
