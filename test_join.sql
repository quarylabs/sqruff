-- Test various join hint combinations
SELECT * FROM table1 FULL OUTER MERGE JOIN table2 ON table1.id = table2.id;
SELECT * FROM table1 LEFT LOOP JOIN table2 ON table1.id = table2.id;
SELECT * FROM table1 INNER HASH JOIN table2 ON table1.id = table2.id;