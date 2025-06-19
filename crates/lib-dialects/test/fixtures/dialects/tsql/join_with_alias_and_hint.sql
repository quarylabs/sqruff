-- Test case for JOIN with alias and table hint
-- This should parse correctly with proper JOIN structure
SELECT *
FROM tableA AS a WITH(NOLOCK)
INNER JOIN tableB AS b WITH(NOLOCK) ON a.id = b.id
LEFT JOIN tableC AS c WITH(READUNCOMMITTED) ON b.id = c.id
WHERE a.value > 0
  AND b.status = 'active'
  AND c.type IS NOT NULL;