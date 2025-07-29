#!/usr/bin/env python3
import subprocess
import sys

# Test SQL with CASE expression
test_sql = """
SELECT 
    CASE 
        WHEN status = 'A' THEN 'Active'
        ELSE 'Unknown'
    END AS status_desc
FROM users;
"""

# Write test SQL to file
with open('test_case_simple.sql', 'w') as f:
    f.write(test_sql)

# Run sqruff parse with T-SQL dialect
try:
    result = subprocess.run(
        ['cargo', 'run', '--', 'lint', 'test_case_simple.sql'],
        capture_output=True,
        text=True,
        env={**subprocess.os.environ, 'SQRUFF_DIALECT': 'tsql'}
    )
    print("STDOUT:")
    print(result.stdout)
    print("\nSTDERR:")
    print(result.stderr)
    print(f"\nReturn code: {result.returncode}")
except Exception as e:
    print(f"Error: {e}")