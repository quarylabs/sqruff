-- Test Position as alias
SELECT * FROM t1 AS Position;
SELECT * FROM t1 JOIN t2 AS Position ON t1.id = Position.id;
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS MyAlias WITH(NOLOCK) ON t1.id = MyAlias.id;
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON t1.id = Position.id;