-- Test CASE expressions in SELECT clauses
SELECT 
    CASE 
        WHEN Status = 'Active' THEN 'A'
        WHEN Status = 'Inactive' THEN 'I'
        ELSE 'U'
    END AS StatusCode,
    CASE Status
        WHEN 'Active' THEN 1
        WHEN 'Inactive' THEN 0
        ELSE -1
    END AS StatusNum
FROM Users;

-- Test CASE in WHERE clause (should work)
SELECT col1 FROM table1 WHERE CASE WHEN col2 = 1 THEN 1 ELSE 0 END = 1;