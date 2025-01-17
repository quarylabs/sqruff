#!/bin/bash
set -eo pipefail

# Extract version from Cargo.toml
cargo_version=$(grep "^version" Cargo.toml | awk -F '"' '{print $2}')

# Optional GitHub release version passed as an argument
github_release_version=$1

# Function to compare two versions
compare_versions() {
    if [ "$1" != "$2" ]; then
        echo "Versions do not match: $1 vs $2"
        exit 1
    fi
}

# Extract version from package.json in VS Code and compare
code_version=$(grep "\"version\"" editors/code/package.json | awk -F '"' '{print $4}')
compare_versions "$cargo_version" "$code_version"

# Extract version from pyproject.toml file
pyproject_version=$(grep "^version" crates/cli/pyproject.toml | awk -F '"' '{print $2}')
compare_versions "$cargo_version" "$pyproject_version"

# If GitHub release version is provided, compare it as well
if [ -n "$github_release_version" ]; then
    compare_versions "$cargo_version" "$github_release_version"
fi


echo "Versions match."

