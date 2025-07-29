#!/bin/bash

echo "=== CASE-related unparsable check ==="
echo "Files with CASE expressions that may have unparsable sections:"
echo ""

# Check for CASE-related unparsable sections
for file in crates/lib-dialects/test/fixtures/dialects/tsql/*.yml; do
    if grep -q "CASE" "$file" 2>/dev/null; then
        if grep -q "unparsable:" "$file" 2>/dev/null; then
            echo "❌ $(basename "$file") - Has CASE and unparsable sections"
        else
            echo "✅ $(basename "$file") - Has CASE, no unparsable sections"
        fi
    fi
done

echo ""
echo "=== Summary of all unparsable sections ==="
./.hacking/scripts/check_for_unparsable.sh | grep -E "Pattern 'unparsable:' found in:|crates/lib-dialects/test/fixtures/dialects/tsql/" | sort