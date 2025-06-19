-- Test Position in different contexts
-- Simple alias - works
SELECT * FROM table1 AS Position;

-- JOIN with simple table - works  
SELECT * FROM t1 JOIN t2 AS Position ON t1.id = Position.id;

-- JOIN with sao.ORDERPOS_P without WITH - works
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS p ON t1.id = p.id;

-- JOIN with sao.ORDERPOS_P with WITH but different alias - works
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS p WITH(NOLOCK) ON t1.id = p.id;

-- The problematic combination: sao.ORDERPOS_P AS Position
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;