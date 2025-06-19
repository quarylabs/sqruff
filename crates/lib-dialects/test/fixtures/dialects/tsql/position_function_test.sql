-- Test POSITION function vs alias
-- POSITION function in T-SQL (actually uses CHARINDEX)
SELECT CHARINDEX('test', 'this is a test');

-- Using Position as an alias should work
SELECT col AS Position FROM table1;

-- The issue: in JOIN context with specific table name
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;