CREATE OR ALTER PROCEDURE Sales.ProcessOrder
    @OrderId INT,
    @Status VARCHAR(20) = 'PENDING'
WITH RECOMPILE, ENCRYPTION
AS
    UPDATE Orders
    SET Status = @Status
    WHERE OrderId = @OrderId