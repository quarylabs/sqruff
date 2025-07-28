#!/usr/bin/env python3

import subprocess
import os

# Get all T-SQL test files
test_dir = 'crates/lib-dialects/test/fixtures/dialects/tsql'
sql_files = [f for f in os.listdir(test_dir) if f.endswith('.sql')]

unparsable_files = []

for file in sql_files:
    file_path = os.path.join(test_dir, file)
    try:
        # Run sqruff lint with a timeout
        result = subprocess.run(
            ['cargo', 'run', '--', 'lint', file_path, '--config', 'test_tsql.sqruff'],
            capture_output=True,
            text=True,
            timeout=30,
            cwd='.'
        )
        
        # Check if there are actual parsing errors (not just style issues)
        if result.returncode != 0:
            stderr_lines = result.stderr.strip().split('\n')
            # Look for actual parsing failures
            parsing_failed = False
            for line in stderr_lines:
                if 'thread main panicked' in line or 'Could not parse' in line or 'Parse Error' in line:
                    parsing_failed = True
                    break
            
            if parsing_failed:
                unparsable_files.append(file)
        
    except subprocess.TimeoutExpired:
        unparsable_files.append(f'{file} (timeout)')
    except Exception as e:
        unparsable_files.append(f'{file} (error: {e})')

print(f'Found {len(sql_files)} T-SQL test files')
print()
print(f'Unparsable files: {len(unparsable_files)}')
for file in sorted(unparsable_files):
    print(f'  - {file}')