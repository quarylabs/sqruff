-- Test case: WITH should NOT be parseable as an alias
SELECT * FROM Users WITH;

-- Test case: WITH should be parseable as table hint
SELECT * FROM Users WITH(NOLOCK);

-- Test case: Regular alias should work
SELECT * FROM Users u;