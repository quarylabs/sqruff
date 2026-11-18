CREATE TABLE example (email TEXT);

SELECT email = trim(email) COLLATE BINARY FROM example;

SELECT email COLLATE NOCASE, name
FROM example
WHERE email COLLATE nocase = 'a@example.com' AND name = 'x' COLLATE RTRIM
GROUP BY email COLLATE NOCASE
ORDER BY email COLLATE BINARY DESC;

SELECT CASE WHEN a COLLATE nocase = b THEN 1 ELSE 0 END FROM t;

SELECT a FROM t WHERE a COLLATE "my collation" = 'x';
