#!/usr/bin/env python3

# Detailed analysis of parsing failures grouped by root cause
root_causes = {
    "CASE Expression Parsing": {
        "description": "CASE expressions in SELECT clauses not properly parsed",
        "files": ["case_in_select.sql", "select.sql", "create_view_with_set_statements.sql"],
        "examples": [
            "CASE WHEN Status = 'Active' THEN 'A' ... END AS StatusCode",
            "CASE WHEN 1 = 1 THEN 'True' WHEN 1 > 1 THEN 'False' ... END",
            "CASE WHEN OLD_VALUE IS NULL THEN 0 ELSE OLD_VALUE END AS OLD_VALUE"
        ],
        "severity": "HIGH",
        "impact": "CASE expressions are fundamental SQL constructs"
    },
    
    "JOIN Hints Support": {
        "description": "T-SQL specific join hints (HASH, MERGE, LOOP) not recognized",
        "files": ["join_hints.sql"],
        "examples": [
            "FULL OUTER MERGE JOIN table2",
            "INNER HASH JOIN table2",
            "LEFT LOOP JOIN table2"
        ],
        "severity": "MEDIUM",
        "impact": "T-SQL specific performance hints"
    },
    
    "CREATE TABLE Syntax": {
        "description": "Various CREATE TABLE constructs not properly parsed",
        "files": ["create_table_constraints.sql", "create_table_with_sequence_bracketed.sql", "temporal_tables.sql"],
        "examples": [
            "CREATE TABLE [dbo].[example](",
            "CREATE TABLE SCHEMA_NAME.TABLE_NAME(",
            "CREATE TABLE Department"
        ],
        "severity": "HIGH",
        "impact": "Basic DDL statements failing to parse"
    },
    
    "Advanced T-SQL Functions": {
        "description": "T-SQL specific functions and syntax not supported",
        "files": ["json_functions.sql", "select_date_functions.sql", "openrowset.sql"],
        "examples": [
            "JSON_ARRAY('a', 1, NULL, 2, NULL ON NULL)",
            "DATEPART(day, [mydate], GETDATE())",
            "OPENROWSET(BULK 'path', FORMAT = 'PARQUET')"
        ],
        "severity": "MEDIUM",
        "impact": "Modern T-SQL features not supported"
    },
    
    "Complex JOIN Patterns": {
        "description": "Multi-table joins with complex ON conditions",
        "files": ["nested_joins.sql"],
        "examples": [
            "ON BA.Iid = I.Bcd",
            "ON I.PID = CAST(P_1.IDEID AS varchar)",
            "LEFT OUTER JOIN (dbo.Test2 AS tst2 INNER JOIN dbo.FilterTable AS fltr1"
        ],
        "severity": "MEDIUM",
        "impact": "Complex query patterns"
    },
    
    "MERGE Statement Syntax": {
        "description": "T-SQL MERGE statement OUTPUT clause",
        "files": ["merge.sql"],
        "examples": [
            "OUTPUT deleted.*, $action, inserted.* INTO #MyTempTable"
        ],
        "severity": "MEDIUM",
        "impact": "MERGE statement completeness"
    },
    
    "VIEW Options": {
        "description": "VIEW creation options not supported",
        "files": ["create_view.sql"],
        "examples": [
            "WITH CHECK OPTION"
        ],
        "severity": "LOW",
        "impact": "View constraint options"
    },
    
    "Trigger Syntax": {
        "description": "T-SQL trigger specific constructs",
        "files": ["triggers.sql"],
        "examples": [
            "FROM inserted AS i",
            "ON DATABASE"
        ],
        "severity": "MEDIUM",
        "impact": "Trigger functionality"
    },
    
    "Table Reference Edge Cases": {
        "description": "Unusual table reference patterns",
        "files": ["table_object_references.sql", "update.sql"],
        "examples": [
            "select column_1 from .[#my_table]",
            "UPDATE stuff SET"
        ],
        "severity": "LOW",
        "impact": "Edge case syntax patterns"
    },
    
    "Window Functions": {
        "description": "Window function syntax issues",
        "files": ["select.sql"],
        "examples": [
            "ROW_NUMBER()OVER(PARTITION BY [EventNM], [PersonID] ORDER BY [DateofEvent] desc)"
        ],
        "severity": "MEDIUM",
        "impact": "Analytics functions missing spacing"
    }
}

print("T-SQL PARSING FAILURE ROOT CAUSE ANALYSIS")
print("=" * 60)

total_files = 16
total_parsing_errors = sum(len(data["files"]) for data in root_causes.values())

print(f"📊 SUMMARY:")
print(f"   • Total unparsable files: {total_files}")
print(f"   • Root cause categories identified: {len(root_causes)}")
print(f"   • All files have genuine parsing failures (not just style issues)")

print(f"\n🔥 SEVERITY BREAKDOWN:")
high_severity = [k for k, v in root_causes.items() if v["severity"] == "HIGH"]
medium_severity = [k for k, v in root_causes.items() if v["severity"] == "MEDIUM"]
low_severity = [k for k, v in root_causes.items() if v["severity"] == "LOW"]

print(f"   • HIGH severity: {len(high_severity)} categories")
print(f"   • MEDIUM severity: {len(medium_severity)} categories")  
print(f"   • LOW severity: {len(low_severity)} categories")

print(f"\n" + "=" * 60)
print("DETAILED ROOT CAUSE ANALYSIS:")
print("=" * 60)

for category, data in root_causes.items():
    severity_emoji = "🔴" if data["severity"] == "HIGH" else "🟡" if data["severity"] == "MEDIUM" else "🟢"
    
    print(f"\n{severity_emoji} {category} [{data['severity']}]")
    print(f"   📝 {data['description']}")
    print(f"   📁 Files affected: {', '.join(data['files'])}")
    print(f"   💥 Impact: {data['impact']}")
    print(f"   🔍 Examples:")
    for example in data['examples'][:2]:  # Show first 2 examples
        print(f"      • {example}")

print(f"\n" + "=" * 60)
print("RECOMMENDATIONS:")
print("=" * 60)

recommendations = [
    {
        "priority": "1. HIGH PRIORITY",
        "items": [
            "Fix CASE expression parsing - affects 3+ files and is fundamental SQL",
            "Fix CREATE TABLE syntax issues - basic DDL must work",
            "Address basic SELECT parsing issues"
        ]
    },
    {
        "priority": "2. MEDIUM PRIORITY", 
        "items": [
            "Add support for T-SQL join hints (HASH, MERGE, LOOP)",
            "Implement T-SQL specific functions (JSON_ARRAY, advanced DATEPART)",
            "Fix complex JOIN pattern parsing",
            "Complete MERGE statement syntax support",
            "Address trigger syntax parsing"
        ]
    },
    {
        "priority": "3. LOW PRIORITY",
        "items": [
            "Add VIEW options support (WITH CHECK OPTION)",
            "Handle edge case table reference patterns",
            "Fix window function spacing issues"
        ]
    }
]

for rec in recommendations:
    print(f"\n{rec['priority']}:")
    for item in rec['items']:
        print(f"   • {item}")

print(f"\n" + "=" * 60)
print("NEXT STEPS:")
print("=" * 60)
print("1. Focus on HIGH priority issues first - they affect fundamental SQL constructs")
print("2. CASE expressions and CREATE TABLE issues should be addressed immediately")
print("3. Consider examining the T-SQL dialect grammar in crates/lib-dialects/src/tsql.rs")
print("4. Test fixes against these specific files to verify improvements")
print("5. Run the unparsable.py script after fixes to track progress toward 100% parsing")