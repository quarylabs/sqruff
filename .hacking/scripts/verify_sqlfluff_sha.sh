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

if ! status=$(curl -sS -o /dev/null -w "%{http_code}" \
    ${GITHUB_TOKEN:+-H "Authorization: token $GITHUB_TOKEN"} \
    "https://api.github.com/repos/sqlfluff/sqlfluff/git/commits/${SHA}"); then
    echo "ERROR: Could not verify SHA ${SHA} in sqlfluff/sqlfluff (request failed)"
    exit 1
fi

case "$status" in
    200) ;;
    404)
        echo "ERROR: SHA ${SHA} does not exist in sqlfluff/sqlfluff (HTTP ${status})"
        exit 1
        ;;
    *)
        echo "ERROR: Could not verify SHA ${SHA} in sqlfluff/sqlfluff (HTTP ${status}); check API access and rate limits"
        exit 1
        ;;
esac

echo "OK: .sqlfluff-sha (${SHA}) verified successfully"
