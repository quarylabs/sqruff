#!/bin/bash
# Check that Bazel's MODULE.bazel.lock is in sync with Cargo.lock
# This verifies that crate_universe dependencies match what Cargo specifies
set -eo pipefail

echo "Checking Bazel and Cargo dependency synchronization..."

# Run bazel mod deps with lockfile_mode=error to check if lock file is up-to-date
# This will fail if MODULE.bazel.lock needs to be regenerated
if ! bazel mod deps --lockfile_mode=error 2>&1; then
    echo ""
    echo "=========================================="
    echo "ERROR: Bazel lock file is out of sync!"
    echo "=========================================="
    echo ""
    echo "The MODULE.bazel.lock file is not in sync with Cargo.lock."
    echo "This can happen when:"
    echo "  - Cargo.lock was updated (cargo update, cargo add, etc.)"
    echo "  - MODULE.bazel was modified"
    echo "  - A Cargo.toml file was modified"
    echo ""
    echo "To fix this, run:"
    echo "  bazel mod deps --lockfile_mode=update"
    echo ""
    echo "Then commit the updated MODULE.bazel.lock file."
    echo ""
    exit 1
fi

echo "Bazel and Cargo dependencies are in sync."
