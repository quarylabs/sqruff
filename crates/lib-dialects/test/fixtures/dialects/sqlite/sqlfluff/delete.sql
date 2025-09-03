DELETE FROM table_name
WHERE a > 0;

DELETE FROM table_name
WHERE a > 0
RETURNING *;

DELETE FROM table_name
WHERE a > 0
RETURNING a;

DELETE FROM table_name
WHERE a > 0
RETURNING a, b AS bee;
