#!/bin/bash
set -eo pipefail

# Extract version from Cargo.toml
cargo_version_cli=$(grep "^version" crates/cli/Cargo.toml | awk -F '"' '{print $2}')
cargo_version_lib=$(grep "^version" crates/lib/Cargo.toml | awk -F '"' '{print $2}')

# Optional GitHub release version passed as an argument
github_release_version=$1

# Function to compare two versions
compare_versions() {
    if [ "$1" != "$2" ]; then
        echo "Versions do not match: $1 vs $2"
        exit 1
    fi
}

compare_versions "$cargo_version_cli" "$cargo_version_lib"

# If GitHub release version is provided, compare it as well
if [ -n "$github_release_version" ]; then
    compare_versions "$cargo_version_cli" "$github_release_version"
fi

echo "Versions match."
