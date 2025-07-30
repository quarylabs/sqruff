CREATE PROCEDURE findjobs @nm sysname = NULL
AS
IF @nm IS NULL
    PRINT 'You must give a user name'
ELSE
    PRINT 'test'