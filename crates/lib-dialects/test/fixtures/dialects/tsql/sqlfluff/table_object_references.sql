-- Test cases from sqlfluff for temp table references
-- These ensure backward compatibility with sqlfluff's parsing

SELECT column_1 FROM [#my_table];

SELECT column_1 FROM dbo.[#my_table];