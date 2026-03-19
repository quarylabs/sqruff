#!/usr/bin/env bash
# Verifies that the SHA in .sqlfluff-sha exists in the sqlfluff GitHub repo.

set -euo pipefail

SHA_FILE=".sqlfluff-sha"

if [ ! -f "$SHA_FILE" ]; then
    echo "ERROR: $SHA_FILE not found"
    exit 1
fi

SHA=$(tr -d '[:space:]' < "$SHA_FILE")

if [ -z "$SHA" ]; then
    echo "ERROR: $SHA_FILE is empty"
    exit 1
fi

if ! echo "$SHA" | grep -qE '^[0-9a-f]{40}$'; then
    echo "ERROR: $SHA_FILE does not contain a valid 40-character hex SHA"
    echo "  Found: $SHA"
    exit 1
fi

status=$(curl -s -o /dev/null -w "%{http_code}" \
    ${GITHUB_TOKEN:+-H "Authorization: token $GITHUB_TOKEN"} \
    "https://api.github.com/repos/sqlfluff/sqlfluff/git/commits/${SHA}")

if [ "$status" != "200" ]; then
    echo "ERROR: SHA ${SHA} does not exist in sqlfluff/sqlfluff (HTTP ${status})"
    exit 1
fi

echo "OK: .sqlfluff-sha (${SHA}) verified successfully"
