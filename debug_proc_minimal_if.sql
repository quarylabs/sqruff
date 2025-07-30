CREATE PROCEDURE test @nm sysname = NULL
AS
IF @nm IS NULL
    PRINT 'Hello';