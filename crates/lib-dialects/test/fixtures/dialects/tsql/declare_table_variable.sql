-- Simple table variable declaration
DECLARE @customers TABLE (CustomerId INT);

-- Table variable with multiple columns
DECLARE @employees TABLE (
    EmployeeId INT PRIMARY KEY,
    Name VARCHAR(100),
    Salary DECIMAL(10,2)
);

-- Table variable with constraints
DECLARE @orders TABLE (
    OrderId INT IDENTITY(1,1) PRIMARY KEY,
    CustomerId INT NOT NULL,
    OrderDate DATETIME DEFAULT GETDATE(),
    Amount DECIMAL(10,2) CHECK (Amount > 0)
);

-- Multiple declarations
DECLARE @var1 INT = 42,
        @var2 VARCHAR(50) = 'test',
        @tableVar TABLE (Id INT, Name VARCHAR(50));

-- Using table variables
INSERT INTO @customers VALUES (1), (2), (3);
SELECT * FROM @customers;