-- Test bare procedure calls
sp_help 'table'
dbo.sp_help 'table'
RAISERROR ('message', 10, 1)
myproc @param = 1