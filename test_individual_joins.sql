-- Test INNER HASH JOIN
SELECT table1.col FROM table1 INNER HASH JOIN table2 ON table1.col = table2.col;

-- Test FULL OUTER HASH JOIN  
SELECT table1.col FROM table1 FULL OUTER HASH JOIN table2 ON table1.col = table2.col;

-- Test LEFT LOOP JOIN
SELECT table1.col FROM table1 LEFT LOOP JOIN table2 ON table1.col = table2.col;