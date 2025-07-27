#!/usr/bin/env python3
import os

# Specific parsing failures found
parsing_failures = {
    "case_in_select.sql": [3, 7],
    "create_table_constraints.sql": [1],
    "create_table_with_sequence_bracketed.sql": [7],
    "create_view.sql": [39],
    "create_view_with_set_statements.sql": [29],
    "join_hints.sql": [10],
    "json_functions.sql": [5],
    "merge.sql": [106],
    "nested_joins.sql": [6, 15, 33],
    "openrowset.sql": [43, 56, 72],
    "select.sql": [3, 20, 84],
    "select_date_functions.sql": [4],
    "table_object_references.sql": [3],
    "temporal_tables.sql": [46],
    "triggers.sql": [24, 48],
    "update.sql": [16]
}

print("Examining specific T-SQL syntax causing parsing failures:")
print("=" * 70)

for filename, line_numbers in parsing_failures.items():
    filepath = f"crates/lib-dialects/test/fixtures/dialects/tsql/{filename}"
    print(f"\n📁 {filename}")
    print("-" * 50)
    
    try:
        with open(filepath, 'r') as f:
            lines = f.readlines()
        
        for line_num in line_numbers:
            if line_num <= len(lines):
                line_content = lines[line_num - 1].rstrip()
                print(f"Line {line_num:3}: {line_content}")
                
                # Also show context (line before and after)
                if line_num > 1:
                    prev_line = lines[line_num - 2].rstrip()
                    print(f"Line {line_num-1:3}: {prev_line} (context)")
                if line_num < len(lines):
                    next_line = lines[line_num].rstrip()
                    print(f"Line {line_num+1:3}: {next_line} (context)")
                print()
    except Exception as e:
        print(f"Error reading file: {e}")

print("\n" + "=" * 70)
print("PATTERN ANALYSIS:")
print("=" * 70)

patterns = {
    "CASE expressions": ["case_in_select.sql"],
    "CREATE TABLE with constraints": ["create_table_constraints.sql"],
    "Sequence/Identity columns": ["create_table_with_sequence_bracketed.sql"],
    "VIEW options (WITH CHECK OPTION)": ["create_view.sql"],
    "SET statements in views": ["create_view_with_set_statements.sql"],
    "JOIN hints (HASH, MERGE, LOOP)": ["join_hints.sql"],
    "JSON functions": ["json_functions.sql"],
    "MERGE statements": ["merge.sql"],
    "Complex JOIN syntax": ["nested_joins.sql"],
    "OPENROWSET functions": ["openrowset.sql"],
    "General SELECT constructs": ["select.sql"],
    "Date/time functions": ["select_date_functions.sql"],
    "Table references": ["table_object_references.sql"],
    "Temporal table features": ["temporal_tables.sql"],
    "Trigger syntax": ["triggers.sql"],
    "UPDATE syntax": ["update.sql"]
}

for category, files in patterns.items():
    print(f"\n🔍 {category}:")
    for file in files:
        if file in parsing_failures:
            print(f"   • {file} - {len(parsing_failures[file])} parsing error(s)")