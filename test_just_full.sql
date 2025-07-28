-- Test if FULL keyword alone causes issues
SELECT * FROM table1 FULL JOIN table2 ON table1.id = table2.id;