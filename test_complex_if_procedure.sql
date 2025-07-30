CREATE PROCEDURE test_proc @nm sysname = NULL
AS
IF @nm IS NULL
    BEGIN
        PRINT 'test'
    END
ELSE
    BEGIN
        PRINT 'other'
    END