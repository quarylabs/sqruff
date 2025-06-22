-- Test various FROM clause patterns
SELECT * FROM Users;
SELECT * FROM Users u;
SELECT * FROM Users AS u;
SELECT * FROM Users WITH (NOLOCK);
SELECT * FROM Users u WITH (NOLOCK);
SELECT * FROM Users AS u WITH (NOLOCK);