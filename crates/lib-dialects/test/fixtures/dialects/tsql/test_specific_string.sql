-- Test if it's the exact string ORDERPOS_P
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS p ON t1.id = p.id;  -- Works
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Pos ON t1.id = Pos.id;  -- Test
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;  -- Fails