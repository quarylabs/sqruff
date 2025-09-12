-- Test identifiers starting with underscore
SELECT * FROM _1.Table;

SELECT * FROM _1._2;

SELECT bar AS _1 FROM foo;

SELECT * FROM foo.bar _1;

SELECT * FROM foo.bar AS _1;

-- Test lowercase identifiers (new pattern: [a-zA-Z_][0-9a-zA-Z_]*)
SELECT * FROM table1;

SELECT column_name FROM my_table;

SELECT col1, col2 FROM lowercase_table;

-- Test mixed case identifiers
SELECT * FROM MyTable;

SELECT ColumnName FROM MixedCaseTable;

-- Test identifiers with numbers
SELECT * FROM table_2;

SELECT column_3 FROM table4;

-- Test identifiers starting with letter (not number)
SELECT abc123 FROM def456;
