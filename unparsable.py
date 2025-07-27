#!/usr/bin/env python3
import subprocess
import glob
import os

# Find all T-SQL test files
tsql_files = glob.glob("crates/lib-dialects/test/fixtures/dialects/tsql/*.sql")
print(f"Found {len(tsql_files)} T-SQL test files")

unparsable_files = []

for sql_file in tsql_files:
    # Run sqruff lint on the file
    result = subprocess.run(
        ["cargo", "run", "--", "lint", sql_file, "--parsing-errors"],
        capture_output=True,
        text=True
    )
    
    # Check if there are parsing errors - check both stdout and stderr
    output = result.stdout + result.stderr
    if "Unparsable section" in output or "????" in output:
        unparsable_files.append(sql_file)

print(f"\nUnparsable files: {len(unparsable_files)}")
for f in sorted(unparsable_files):
    print(f"  - {os.path.basename(f)}")