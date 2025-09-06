-- Wildcard EXCEPT clause
SELECT * EXCEPT (password) FROM users;

-- Multiple columns in EXCEPT
SELECT * EXCEPT (password, email) FROM users;

-- EXCEPT with alias
SELECT u.* EXCEPT (password) FROM users u;