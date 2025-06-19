-- Test sao.ORDERPOS_P in JOIN
SELECT * FROM dbo.table1 AS t1
INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON t1.id = Position.id;