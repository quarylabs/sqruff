-- Test combining temp tables (# at start) with SQL Server 2017+ identifiers (# at end)

-- Regular identifiers with # at end
SELECT * FROM orders#;

-- Temp tables with # at start
SELECT * FROM #temp_orders;
SELECT * FROM ##global_orders;

-- Mixed usage
SELECT 
    o#.id#,
    t.order_id
FROM orders# o#
CROSS JOIN #temp_orders t;