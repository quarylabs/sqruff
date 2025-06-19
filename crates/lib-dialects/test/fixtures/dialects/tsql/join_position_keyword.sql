-- Test if Position is a keyword issue
SELECT * FROM dbo.table1 AS t1
INNER JOIN sao.ORDERPOS_P AS MyAlias WITH(NOLOCK) ON t1.id = MyAlias.id;