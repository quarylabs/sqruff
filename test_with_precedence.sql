-- Test T-SQL table hints with precedence-based parsing
SELECT * FROM Users WITH(NOLOCK);
SELECT * FROM Users u WITH(NOLOCK);
SELECT * FROM Users AS u WITH(NOLOCK);
SELECT * FROM Users u;
SELECT * FROM Users;