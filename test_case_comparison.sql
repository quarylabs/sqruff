-- Test 1: CASE in WHERE (works)
SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1;

-- Test 2: CASE in SELECT (fails)  
SELECT CASE WHEN 1=1 THEN 'A' END FROM Users;