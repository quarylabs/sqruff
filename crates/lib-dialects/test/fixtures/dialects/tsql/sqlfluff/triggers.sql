CREATE OR ALTER TRIGGER reminder1
ON Sales.Customer
AFTER INSERT, UPDATE
AS SELECT 1;
GO

CREATE TRIGGER reminder
ON person.address
AFTER UPDATE
AS
IF (UPDATE(stateprovinceid) OR UPDATE(postalcode))
    BEGIN
        RAISERROR (50009, 16, 10)
    END;
GO
