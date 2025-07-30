#!/bin/bash
echo "Checking for unparsable T-SQL files..."
find . -name "*.yml" -path "*tsql*" -exec grep -l "unparsable:" {} \; | sort
echo ""
echo "Count of unparsable files:"
find . -name "*.yml" -path "*tsql*" -exec grep -l "unparsable:" {} \; | wc -l