-- Test 1: Simple CASE without alias
SELECT CASE WHEN 1=1 THEN 'A' END

-- Test 2: CASE with AS alias  
SELECT CASE WHEN 1=1 THEN 'A' END AS test

-- Test 3: Simple column
SELECT col1

-- Test 4: Column with alias
SELECT col1 AS test

-- Test 5: T-SQL equals alias
SELECT test = col1