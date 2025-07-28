-- Test 1: Simple FULL OUTER JOIN (should work)
SELECT * FROM table1 FULL OUTER JOIN table2 ON table1.id = table2.id;

-- Test 2: FULL OUTER MERGE JOIN
SELECT * FROM table1 FULL OUTER MERGE JOIN table2 ON table1.id = table2.id;

-- Test 3: LEFT LOOP JOIN
SELECT * FROM table1 LEFT LOOP JOIN table2 ON table1.id = table2.id;