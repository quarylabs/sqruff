#!/usr/bin/env python3
import subprocess
import os

# List of unparsable files from the original script
unparsable_files = [
    "case_in_select.sql",
    "create_table_constraints.sql", 
    "create_table_with_sequence_bracketed.sql",
    "create_view.sql",
    "create_view_with_set_statements.sql",
    "join_hints.sql",
    "json_functions.sql",
    "merge.sql",
    "nested_joins.sql",
    "openrowset.sql",
    "select.sql",
    "select_date_functions.sql",
    "table_object_references.sql",
    "temporal_tables.sql",
    "triggers.sql",
    "update.sql"
]

print("Analyzing T-SQL parsing errors for unparsable files:")
print("=" * 60)

analysis_results = {}

for filename in unparsable_files:
    filepath = f"crates/lib-dialects/test/fixtures/dialects/tsql/{filename}"
    print(f"\n📁 FILE: {filename}")
    print("-" * 40)
    
    # Run sqruff lint with parsing errors
    result = subprocess.run(
        ["cargo", "run", "--", "lint", filepath, "--parsing-errors"],
        capture_output=True,
        text=True
    )
    
    output = result.stdout + result.stderr
    lines = output.split('\n')
    
    # Extract only parsing errors (marked with ????)
    parsing_errors = []
    style_errors = []
    
    for line in lines:
        if "????" in line:
            parsing_errors.append(line.strip())
        elif "|" in line and ("LT" in line or "CP" in line or "RF" in line):
            # Count style/linting errors but don't show them all
            style_errors.append(line.strip())
    
    analysis_results[filename] = {
        'parsing_errors': parsing_errors,
        'style_error_count': len(style_errors)
    }
    
    if parsing_errors:
        print("🔴 PARSING ERRORS:")
        for error in parsing_errors:
            print(f"  {error}")
    else:
        print("✅ NO PARSING ERRORS - only style issues")
    
    if style_errors:
        print(f"⚠️  STYLE/LINTING ISSUES: {len(style_errors)} total")

print("\n" + "=" * 60)
print("SUMMARY:")
print("=" * 60)

true_parsing_failures = []
style_only_issues = []

for filename, results in analysis_results.items():
    if results['parsing_errors']:
        true_parsing_failures.append(filename)
        print(f"🔴 {filename}: {len(results['parsing_errors'])} parsing errors")
    else:
        style_only_issues.append(filename)
        print(f"✅ {filename}: Only {results['style_error_count']} style issues")

print(f"\n📊 ANALYSIS RESULTS:")
print(f"   • Files with TRUE PARSING FAILURES: {len(true_parsing_failures)}")
print(f"   • Files with ONLY STYLE/LINTING ISSUES: {len(style_only_issues)}")
print(f"   • Total files analyzed: {len(unparsable_files)}")

if true_parsing_failures:
    print(f"\n🎯 PRIORITY FILES TO FIX (true parsing failures):")
    for filename in true_parsing_failures:
        print(f"   • {filename}")

if style_only_issues:
    print(f"\n📝 Files that parse correctly (just style issues):")
    for filename in style_only_issues:
        print(f"   • {filename}")