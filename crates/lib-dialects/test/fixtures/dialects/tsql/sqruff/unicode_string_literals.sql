SELECT N'abc' AS a, N'' AS empty, N'It''s' AS escaped, N'Michél ☃' AS unicode;
SELECT N'first line
second line' AS multiline, N'C:\path\' AS backslash;
SELECT name FROM dbo.t WHERE name = N'abc' AND other = N'It''s';
DECLARE @v NVARCHAR(10) = N'abc';
SET @v = N'def';
EXEC dbo.example @value = N'abc';
PRINT N'abc';
