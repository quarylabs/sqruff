-- SET EXCEPT operation
SELECT 1 EXCEPT SELECT 2;

-- SET EXCEPT ALL operation
SELECT 1 EXCEPT ALL SELECT 2;

-- SET EXCEPT with complex queries
SELECT id, name FROM users EXCEPT SELECT id, name FROM deleted_users;