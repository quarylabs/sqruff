CREATE PROCEDURE test_proc @nm sysname = NULL
AS
IF @nm IS NULL PRINT 'test'