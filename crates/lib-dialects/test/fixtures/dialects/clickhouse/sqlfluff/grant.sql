-- Basic privileges
GRANT SELECT ON db_name TO user_name;
GRANT SELECT (col_a, col_b) ON db_name TO CURRENT_USER;
GRANT INSERT ON db_name.* TO user_name;
GRANT SELECT ON *.* TO user_name;
GRANT SELECT ON db_name.table_prefix* TO user_name;
GRANT SELECT ON db_prefix*.* TO user_name;
GRANT DELETE ON db_name TO user_name;
GRANT UPDATE ON db_name TO user_name;

-- ALTER privileges
GRANT ALTER ON db_name TO user_name;
GRANT ALTER TABLE ON db_name TO user_name;
GRANT ALTER DELETE ON db_name TO user_name;
GRANT ALTER UPDATE ON db_name TO user_name;
GRANT ALTER ADD COLUMN ON db_name TO user_name;
GRANT ALTER DROP COLUMN ON db_name TO user_name;
GRANT ALTER VIEW REFRESH ON db_name TO user_name;
GRANT ALTER VIEW MODIFY QUERY ON db_name TO user_name;

-- CREATE privileges
GRANT CREATE ON db_name TO user_name;
GRANT CREATE TABLE ON db_name TO user_name;
GRANT CREATE ROW POLICY ON db_name TO user_name;
GRANT ALTER QUOTA ON db_name TO user_name;
GRANT DROP SETTINGS PROFILE ON db_name TO user_name;

-- Multiple privileges
GRANT SELECT, INSERT ON db_name TO user_name;
GRANT ALTER DELETE, ALTER UPDATE ON db_name TO user_name;

-- ACCESS privileges
GRANT SHOW ACCESS ON db_name TO user_name;
GRANT ROLE ADMIN ON db_name TO user_name;
GRANT ACCESS MANAGEMENT ON db_name TO user_name;

-- ALL PRIVILEGES
GRANT ALL ON db_name TO user_name;

-- REVOKE privileges
REVOKE SELECT ON db_name FROM user_name;
