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

pattern="unparsable:"
shift  # Shift so $@ now contains only filenames (if any)

# If no files are specified, default to looking in crates/lib-dialects/test/fixtures/dialects/***/*.yml
if [[ $# -eq 0 ]]; then
  set -- crates/lib-dialects/test/fixtures/dialects/**\/*.yml
fi

found=0
files_found=()

for filename in "$@"; do
  # Use extended regular expressions (-E). -q ensures grep is quiet (no output).
  if grep -qE "$pattern" "$filename" 2>/dev/null; then
    found=1
    files_found+=("$filename")
  fi
done

# If found in any file, list them
if [[ $found -eq 1 ]]; then
  echo "Pattern '$pattern' found in:"
  for file in "${files_found[@]}"; do
    echo "  $file"
  done
fi

# Exit with 1 if found, otherwise 0
exit $found