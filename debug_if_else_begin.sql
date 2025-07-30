CREATE PROCEDURE test @nm sysname = NULL
AS
IF @nm IS NULL
BEGIN
    PRINT 'Null'
END
ELSE
BEGIN
    PRINT 'Not null'
END;