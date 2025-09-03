-- Table variable with complex constraints (IDENTITY, DEFAULT, CHECK)
-- Created for testing T-SQL constraint parsing enhancements
DECLARE @orders TABLE (
    OrderId INT IDENTITY(1,1) PRIMARY KEY,
    CustomerId INT NOT NULL,
    OrderDate DATETIME DEFAULT GETDATE(),
    Amount DECIMAL(10,2) CHECK (Amount > 0)
);