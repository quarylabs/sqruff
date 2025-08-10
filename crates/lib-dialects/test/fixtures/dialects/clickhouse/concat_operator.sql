-- Test || concatenation operator as single token
SELECT 'hello' || ' ' || 'world' AS greeting;

-- Test || in complex expressions
SELECT 
    first_name || ' ' || last_name AS full_name,
    city || ', ' || country AS location
FROM users;

-- Test || with functions
SELECT UPPER(prefix || '_' || suffix) AS code;

-- Test || in WHERE clause
SELECT * FROM products 
WHERE category || '_' || subcategory = 'electronics_phones';

-- Test || in CASE expressions
SELECT 
    CASE 
        WHEN status = 'active' THEN 'User: ' || username
        ELSE 'Inactive: ' || username || ' (since ' || date || ')'
    END AS user_status
FROM accounts;

-- Test || with NULL handling
SELECT COALESCE(first_name || ' ' || last_name, 'Unknown') AS name;

-- Test multiple || in single expression
SELECT col1 || col2 || col3 || col4 || col5 AS concatenated;