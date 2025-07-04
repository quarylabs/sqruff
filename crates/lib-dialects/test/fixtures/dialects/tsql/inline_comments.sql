-- Test that # is NOT treated as inline comment in T-SQL
-- Only -- should start inline comments

-- The following should parse correctly
SELECT column# FROM table#;