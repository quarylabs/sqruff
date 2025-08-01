-- Test trigger body parsing
CREATE TRIGGER safety
ON DATABASE
FOR DROP_SYNONYM
AS
   IF (@@ROWCOUNT = 0)
   RETURN;
   RAISERROR ('message', 10, 1)
   ROLLBACK
GO