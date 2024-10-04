#!/bin/bash
set -eo pipefail

VERSION="v$(grep "^version" Cargo.toml | awk -F '"' '{print $2}')"
echo "VERSION: $VERSION"

repo="quarylabs/sqrufff"

curl -L -o extension.vsix https://github.com/${repo}/releases/download/v"${VERSION}"/editors/code/sqruff-"${VERSION}".vsix