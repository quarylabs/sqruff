-- Test LOOP JOIN
SELECT * FROM table1 LOOP JOIN table2 ON table1.id = table2.id;

-- Test FULL OUTER LOOP JOIN
SELECT * FROM table1 FULL OUTER LOOP JOIN table2 ON table1.id = table2.id;