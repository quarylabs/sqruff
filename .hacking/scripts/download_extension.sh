#!/bin/bash
set -eo pipefail

VERSION="$(grep "^version" Cargo.toml | awk -F '"' '{print $2}')"
echo "VERSION: $VERSION"

repo="quarylabs/sqruff"

URL="https://github.com/${repo}/releases/download/v${VERSION}/sqruff-${VERSION}.vsix"
echo "URL: $URL"

curl -fL -o extension.vsix "$URL"
