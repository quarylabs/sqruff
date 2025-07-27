-- Test 1: Simple CASE expression in SELECT
SELECT CASE WHEN 1=1 THEN 'A' END;

-- Test 2: Alias assignment with CASE (should work)
SELECT StatusCode = CASE WHEN Status = 'Active' THEN 'A' END;

-- Test 3: CASE in WHERE (should work)
SELECT * FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1;