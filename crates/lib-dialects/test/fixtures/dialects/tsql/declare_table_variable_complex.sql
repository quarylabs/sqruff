-- Table variable with constraints from original SQLFluff tests
DECLARE @orders TABLE (
    OrderId INT IDENTITY(1,1) PRIMARY KEY,
    CustomerId INT NOT NULL,
    OrderDate DATETIME DEFAULT GETDATE(),
    Amount DECIMAL(10,2) CHECK (Amount > 0)
);