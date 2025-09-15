#!/usr/bin/env bash
#
# Usage:
#   ./check_pattern.sh "your_grep_pattern" [file1 file2 file3 ...]
#
# Examples:
#   # Use the default directory if no files are provided
#   ./check_pattern.sh "unparsable:"
#
#   # Provide explicit files or directories
#   ./check_pattern.sh "error|fail" logs/*.log
#
# Exit codes:
#   0 - Pattern NOT found in any file
#   1 - Pattern found in at least one file

# Pattern is not used from command line arguments in this script
# The patterns are hardcoded as pattern_unparsable and pattern_file

# If no files are specified, default to looking in crates/lib-dialects/test/fixtures/dialects/**/*.yml
if [[ $# -eq 0 ]]; then
  # Use find to get all .yml files recursively
  set -- $(find crates/lib-dialects/test/fixtures/dialects -name "*.yml" -type f)
fi

pattern_unparsable="unparsable:"
files_found_unparsable=()

pattern_file="\- file:"
files_found_file=()

found=0

for filename in "$@"; do
  # Use extended regular expressions (-E). -q ensures grep is quiet (no output).
  if grep -qE "$pattern_unparsable" "$filename" 2>/dev/null; then
    found=1
    files_found_unparsable+=("$filename")
  fi
  if grep -qE "$pattern_file" "$filename" 2>/dev/null; then
    found=1
    files_found_file+=("$filename")
  fi
done

# If found in any file, list them
if [[ $found -eq 1 ]]; then
  echo "Pattern '$pattern_unparsable' found in:"
  for file in "${files_found_unparsable[@]}"; do
    echo "  $file"
  done
  echo "Pattern '$pattern_file' found in:"
  for file in "${files_found_file[@]}"; do
    echo "  $file"
  done
fi

# Exit with 1 if found, otherwise 0
exit $found
