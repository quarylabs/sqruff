-- Comprehensive test combining sqlfluff temp table syntax with SQL Server 2017+ # suffix syntax
-- This ensures both features work together without conflicts

-- Traditional temp tables (# at start) - sqlfluff compatible
CREATE TABLE #temp_orders (id INT);
CREATE TABLE ##global_temp (id INT);

-- Quoted temp tables - sqlfluff test cases
SELECT * FROM ."#my_table";
SELECT * FROM .[#my_table];
SELECT * FROM ..[#quoted_temp];
SELECT * FROM dbo.[#temp_table];

-- SQL Server 2017+ syntax (# at end) - our extension
CREATE TABLE orders# (
    order_id# INT,
    total# DECIMAL(10,2)
);

-- Mixed usage showing both work together
SELECT 
    o#.order_id#,         -- # at end
    t.id                  -- from temp table
FROM orders# o#          -- # at end
CROSS JOIN #temp_orders t; -- # at start

-- Inline comments should work (only --, not #)
SELECT * FROM orders# -- This is a comment
WHERE total# > 100; -- # in comment is fine

-- Block comments with # inside
SELECT /* Using table# */ * FROM orders#;

-- Cleanup
DROP TABLE orders#;
DROP TABLE #temp_orders;
DROP TABLE ##global_temp;