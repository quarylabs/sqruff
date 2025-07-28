-- Test HASH JOIN without type
SELECT * FROM table1 HASH JOIN table2 ON table1.id = table2.id;