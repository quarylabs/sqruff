SELECT @variable
FROM table1
WHERE @variable = 1;

SELECT ?2
FROM table1
WHERE ?2 = 1;

SELECT :variable
FROM table1
WHERE :variable = 1;

SELECT $variable
FROM table1
WHERE $variable = 1;

SELECT @variable
FROM table1
GROUP BY @variable
HAVING $variable = 1;

SELECT ? from table1 where ? = 1;

-- Bind parameters directly after a compound comparison operator must not
-- split the operator (e.g. `>=` into `> =`). See issue #2624.
SELECT id, title, published_at
FROM posts
WHERE published_at >= @since;

SELECT id
FROM posts
GROUP BY id
HAVING count(id) >= @threshold;
