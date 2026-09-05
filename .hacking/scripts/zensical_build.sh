#!/bin/bash
# Verify the cached Zensical build produced a complete site.
set -euo pipefail

DOCS_DIR="$RUNFILES_DIR/$DOCS_SITE"

for expected_file in index.html search.json sitemap.xml; do
    if [[ ! -f "$DOCS_DIR/$expected_file" ]]; then
        echo "Missing expected documentation output: $expected_file" >&2
        exit 1
    fi
done

echo "Documentation build contains the expected site outputs."
