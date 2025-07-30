-- Test with IF on same line as AS
CREATE PROCEDURE test1
AS IF 1 = 1 SELECT 1

-- Test with IF on new line after AS
CREATE PROCEDURE test2
AS
IF 1 = 1 SELECT 1