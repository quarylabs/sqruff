-- Test CASE expressions in different contexts

-- Simple CASE in SELECT
SELECT 
    CASE 
        WHEN status = 'A' THEN 'Active'
        WHEN status = 'I' THEN 'Inactive'
        ELSE 'Unknown'
    END AS status_desc
FROM users;

-- CASE in WHERE clause (this works fine)
SELECT * FROM users
WHERE CASE 
    WHEN role = 'admin' THEN 1
    ELSE 0
END = 1;

-- CASE in IF statement (needs END as terminator)
IF EXISTS (SELECT 1 FROM users WHERE active = 1)
    SELECT 
        CASE status
            WHEN 'A' THEN 'Active'
            ELSE 'Inactive'
        END AS status
    FROM users;

-- BEGIN...END block (needs END as terminator)
BEGIN
    SELECT 
        CASE 
            WHEN amount > 1000 THEN 'High'
            ELSE 'Low'
        END AS category
    FROM transactions;
END;