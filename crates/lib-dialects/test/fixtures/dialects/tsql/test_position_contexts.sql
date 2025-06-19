-- Test POSITION in various contexts
-- As column alias
SELECT id AS Position FROM t1;

-- As table alias in FROM
SELECT * FROM t1 AS Position;

-- As table alias in simple JOIN
SELECT * FROM t1 JOIN t2 AS Position ON t1.id = Position.id;

-- The failing case
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;