-- Test case for JOIN with schema-qualified tables, aliases and table hints
SELECT *
FROM dbo.tableA AS a WITH(NOLOCK)
INNER JOIN dbo.tableB AS b WITH(NOLOCK) ON a.id = b.id
WHERE a.value > 0;