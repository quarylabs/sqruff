-- First, test if plain JOIN works
SELECT * FROM table1 JOIN table2 ON table1.id = table2.id;

-- Then test HASH JOIN
SELECT * FROM table1 HASH JOIN table2 ON table1.id = table2.id;

-- Then test INNER HASH JOIN
SELECT * FROM table1 INNER HASH JOIN table2 ON table1.id = table2.id;