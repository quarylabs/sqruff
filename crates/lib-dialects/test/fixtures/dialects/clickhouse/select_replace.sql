-- REPLACE clause
SELECT * REPLACE (new_value AS old_column) FROM users;

-- Multiple columns in REPLACE
SELECT * REPLACE (new_value AS old_column, 'Anonymous' AS name) FROM users;