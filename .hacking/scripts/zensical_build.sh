#!/bin/bash
# Check the cached documentation artifact used by the website.
set -euo pipefail

SITE="$RUNFILES_DIR/$DOCS_SITE"
test -s "$SITE/index.html"
echo "Documentation built successfully: $SITE"
