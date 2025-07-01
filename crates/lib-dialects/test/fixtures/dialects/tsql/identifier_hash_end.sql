-- SQL Server 2017+ allows # at the end of identifiers
-- This test validates that the T-SQL dialect correctly lexes these identifiers as single tokens

-- CREATE TABLE with # at end
CREATE TABLE orders# (
    order_id# INT PRIMARY KEY,
    customer_id# INT NOT NULL,
    total# DECIMAL(10,2)
);

-- INSERT with # identifiers
INSERT INTO orders# (order_id#, customer_id#, total#)
VALUES (1, 100, 99.99);

-- SELECT with various # identifiers
SELECT 
    o#.order_id# as id#,
    o#.customer_id# as cust#,
    o#.total# as amount#
FROM orders# as o#
WHERE o#.total# > 50;

-- JOIN with # identifiers
SELECT 
    o#.order_id#,
    c#.name#
FROM orders# o#
INNER JOIN customers# c# ON o#.customer_id# = c#.customer_id#;

-- CTE with # identifiers
WITH order_summary# AS (
    SELECT 
        customer_id# as cid#,
        SUM(total#) as sum_total#
    FROM orders#
    GROUP BY customer_id#
)
SELECT * FROM order_summary#;

-- UPDATE with # identifiers
UPDATE orders#
SET total# = total# * 1.1
WHERE customer_id# = 100;

-- DELETE with # identifiers
DELETE FROM orders# 
WHERE order_id# = 1;

-- DROP TABLE
DROP TABLE orders#;

-- Temp tables should still work (# at start)
CREATE TABLE #temp_orders (
    id INT
);

-- Global temp tables (## at start)
CREATE TABLE ##global_temp_orders (
    id INT
);

-- These should be treated as separate tokens (not valid SQL Server syntax)
-- But included to ensure lexer doesn't get confused
-- SELECT alias# # FROM table#;  -- Two separate tokens: alias# and #