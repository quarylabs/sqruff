-- Simple test for alias with table hint
SELECT *
FROM mytable AS t WITH(NOLOCK)
WHERE t.id = 1;