#!/bin/bash
# compare-baselines.sh - Compare two saved benchmark baselines
#
# Usage: ./compare-baselines.sh <package> <baseline1> <baseline2>
#
# Example:
#   ./compare-baselines.sh sqruff-lib before after

set -e

PACKAGE="$1"
BASELINE1="$2"
BASELINE2="$3"

if [[ -z "$PACKAGE" || -z "$BASELINE1" || -z "$BASELINE2" ]]; then
    echo "Usage: $0 <package> <baseline1> <baseline2>"
    echo ""
    echo "Compares two saved benchmark baselines and shows the difference."
    echo ""
    echo "Example:"
    echo "  $0 sqruff-lib before after"
    exit 1
fi

BASELINE_DIR="target/criterion"

echo "=== Comparing Baselines ==="
echo "Package: $PACKAGE"
echo "Baseline 1: $BASELINE1"
echo "Baseline 2: $BASELINE2"
echo ""

# Find all benchmark estimate files
if [[ -d "$BASELINE_DIR" ]]; then
    echo "=== Results ==="
    echo ""

    find "$BASELINE_DIR" -name "estimates.json" -path "*$BASELINE2*" | while read -r file; do
        benchmark=$(echo "$file" | sed "s|$BASELINE_DIR/||" | cut -d'/' -f1)

        # Get corresponding file from baseline1
        file1=$(echo "$file" | sed "s|$BASELINE2|$BASELINE1|")

        if [[ -f "$file1" ]]; then
            # Extract mean values (simplified - criterion stores complex JSON)
            mean2=$(grep -o '"point_estimate":[0-9.]*' "$file" | head -1 | cut -d':' -f2)
            mean1=$(grep -o '"point_estimate":[0-9.]*' "$file1" | head -1 | cut -d':' -f2)

            if [[ -n "$mean1" && -n "$mean2" ]]; then
                # Calculate percentage change
                change=$(echo "scale=2; (($mean2 - $mean1) / $mean1) * 100" | bc 2>/dev/null || echo "N/A")

                echo "Benchmark: $benchmark"
                echo "  $BASELINE1: ${mean1}ns"
                echo "  $BASELINE2: ${mean2}ns"
                echo "  Change: ${change}%"
                echo ""
            fi
        fi
    done
else
    echo "No benchmark results found in $BASELINE_DIR"
    echo "Run benchmarks first with --save-baseline"
fi
