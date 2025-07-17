-- Test 1: Simple SELECT with OPTION (this works)
SELECT 1 OPTION (MAXDOP 1);

-- Test 2: UNION with OPTION (this should work)
SELECT 1 UNION SELECT 2 OPTION (MERGE UNION);

-- Test 3: UNION with parentheses
(SELECT 1 UNION SELECT 2) OPTION (MERGE UNION);