CREATE PROCEDURE findjobs @nm sysname = NULL
AS
IF @nm IS NULL
BEGIN
    PRINT 'You must give a user name'
    RETURN
END
ELSE
BEGIN
    SELECT 1
END;