#!/bin/bash
# Check that Bazel's MODULE.bazel.lock is in sync with Cargo.lock
# This verifies that crate_universe dependencies match what Cargo specifies
set -eo pipefail

echo "Checking Bazel and Cargo dependency synchronization..."

# Regenerate the lockfile
bazel mod deps --lockfile_mode=update

# Check if there are any changes
if ! git diff --quiet MODULE.bazel.lock; then
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
    echo "The lockfile has been updated. Please commit the changes:"
    echo "  git add MODULE.bazel.lock"
    echo "  git commit -m 'chore: update MODULE.bazel.lock'"
    echo ""
    exit 1
fi

echo "Bazel and Cargo dependencies are in sync."
