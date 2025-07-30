CREATE PROCEDURE test @nm sysname = NULL
AS
IF @nm IS NULL
    PRINT 'Null'
ELSE
    PRINT 'Not null';