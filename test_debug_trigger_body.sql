-- Test simplified trigger
CREATE TRIGGER test
ON DATABASE
FOR DROP_SYNONYM
AS
RAISERROR ('message', 10, 1)