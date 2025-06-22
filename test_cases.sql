-- Case 1: WITH as table hint (should work)
SELECT * FROM Users WITH(NOLOCK);

-- Case 2: WITH as alias (should fail - reserved keyword)
SELECT * FROM Users WITH;

-- Case 3: Normal alias (should work)
SELECT * FROM Users u;

-- Case 4: AS alias with hint (should work)
SELECT * FROM Users AS u WITH(NOLOCK);