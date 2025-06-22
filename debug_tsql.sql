-- Test case 1: WITH as a table hint (should work)
SELECT * FROM Users WITH(NOLOCK);

-- Test case 2: WITH as an alias (should fail because WITH is reserved)
SELECT * FROM Users WITH;

-- Test case 3: Normal alias (should work)
SELECT * FROM Users u;

-- Test case 4: AS alias with hint (should work)
SELECT * FROM Users AS u WITH(NOLOCK);

-- Test case 5: Naked alias with hint (should work)
SELECT * FROM Users u WITH(NOLOCK);