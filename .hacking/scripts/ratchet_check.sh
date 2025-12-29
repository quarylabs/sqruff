#!/usr/bin/env bash
# Checks that GitHub Actions workflow versions are pinned using ratchet

set -euo pipefail

# Find the ratchet binary in runfiles (bzlmod naming)
RATCHET="${RUNFILES_DIR:-$0.runfiles}/+_repo_rules+ratchet/ratchet"

if [[ ! -x "$RATCHET" ]]; then
    echo "ERROR: ratchet binary not found at $RATCHET"
    exit 1
fi

# Run ratchet lint on all workflow files
"$RATCHET" lint .github/workflows/*.yml
