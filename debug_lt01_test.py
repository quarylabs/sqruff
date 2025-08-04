#!/usr/bin/env python3

import subprocess
import tempfile
import os

def test_lt01_case():
    # Test case from LT01-excessive.yml test_identifier_fix
    fail_sql = """SELECT [thistable] . [col]
FROM [thisdatabase] . [thisschema]
        . [thistable]"""
    
    expected_sql = """SELECT [thistable].[col]
FROM [thisdatabase].[thisschema].[thistable]"""
    
    # Create temp files
    with tempfile.NamedTemporaryFile(mode='w', suffix='.sql', delete=False) as sql_file:
        sql_file.write(fail_sql)
        sql_path = sql_file.name
    
    config_content = "[sqruff]\ndialect = tsql"
    with tempfile.NamedTemporaryFile(mode='w', suffix='.sqruff', delete=False) as config_file:
        config_file.write(config_content)
        config_path = config_file.name
    
    try:
        # Run sqruff fix
        result = subprocess.run([
            'cargo', 'run', '--', 'fix', sql_path, '--config', config_path
        ], capture_output=True, text=True, cwd='/home/fank/repo/sqruff')
        
        # Read the result
        with open(sql_path, 'r') as f:
            actual_sql = f.read().strip()
        
        print("ORIGINAL SQL:")
        print(repr(fail_sql))
        print("\nEXPECTED SQL:")
        print(repr(expected_sql))
        print("\nACTUAL SQL:")
        print(repr(actual_sql))
        print("\nTEST PASSED:", actual_sql == expected_sql)
        print("\nSTDERR:")
        print(result.stderr)
        
        if actual_sql != expected_sql:
            print("\nDIFF:")
            print("Expected lines:", expected_sql.split('\n'))
            print("Actual lines:  ", actual_sql.split('\n'))
        
    finally:
        os.unlink(sql_path)
        os.unlink(config_path)

if __name__ == "__main__":
    test_lt01_case()