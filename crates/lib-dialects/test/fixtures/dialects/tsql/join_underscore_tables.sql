-- Test various table name patterns
SELECT * FROM table_a AS a WITH(NOLOCK)
INNER JOIN TABLE_B AS b WITH(NOLOCK) ON a.id = b.id
INNER JOIN table_c_d AS c WITH(NOLOCK) ON b.id = c.id
INNER JOIN ORDERS_P AS o WITH(NOLOCK) ON c.id = o.id;