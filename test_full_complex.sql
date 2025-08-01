CREATE TRIGGER test_trigger ON test_table
AFTER INSERT
AS
IF (ROWCOUNT_BIG() = 0)
RETURN;
IF EXISTS (SELECT 1
           FROM inserted AS i
           JOIN Purchasing.Vendor AS v
           ON v.BusinessEntityID = i.VendorID
           WHERE v.CreditRating = 5
          )
BEGIN
PRINT 'Found';
END;
GO