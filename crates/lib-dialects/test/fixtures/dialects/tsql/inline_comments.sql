-- Test that # is NOT treated as inline comment in T-SQL
-- Only -- should start inline comments

-- This is a valid comment
SELECT 1; -- This is also a valid comment

-- The following should parse correctly, # is NOT a comment
SELECT column# FROM table#; -- # in identifier is fine
SELECT #temp.id FROM #temp; -- # starts temp table name

-- Note: # at beginning of line is not a comment in T-SQL
-- If it were, the temp table syntax wouldn't work

-- Multiple # symbols
SELECT a#, b#, c# FROM table#;

-- # in strings is just text
SELECT 'This # is not a comment' as text;

-- Edge case: identifier# followed by comment
SELECT total# -- The # before this comment is part of the identifier
FROM orders#;