#!/usr/bin/env bash
# Checks that files in editors/code match their root counterparts

set -euo pipefail

check_match() {
    local root_file="$1"
    local copy_file="$2"

    if ! diff -q "$root_file" "$copy_file" > /dev/null 2>&1; then
        echo "ERROR: $copy_file does not match $root_file"
        echo "Please copy $root_file to $copy_file"
        return 1
    fi
    echo "OK: $copy_file matches $root_file"
}

exit_code=0

check_match "LICENSE" "editors/code/LICENSE" || exit_code=1
check_match "README.md" "editors/code/README.md" || exit_code=1

exit $exit_code
