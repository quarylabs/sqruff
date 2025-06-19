-- Basic T-SQL syntax tests

-- TOP clause
SELECT TOP 10 * FROM customers;
SELECT TOP (100) PERCENT * FROM orders;
SELECT TOP 5 WITH TIES * FROM products ORDER BY price;

-- Variable declarations
DECLARE @counter INT = 0;
DECLARE @name VARCHAR(50);

-- SET statements
SET @counter = 10;
SET @name = 'John Doe';

-- PRINT statement
PRINT 'Hello T-SQL';
PRINT @counter;

-- Control flow
IF @counter > 5
BEGIN
    PRINT 'Counter is greater than 5';
END
ELSE
BEGIN
    PRINT 'Counter is 5 or less';
END

-- WHILE loop
WHILE @counter < 20
BEGIN
    SET @counter = @counter + 1;
END

-- Table hints
SELECT * FROM customers WITH(NOLOCK);
SELECT * FROM orders WITH(READUNCOMMITTED);

-- Square bracket identifiers
SELECT [Customer ID], [First Name] FROM [Customer Table];

-- GO batch separator
CREATE TABLE test_table (id INT);
GO
INSERT INTO test_table VALUES (1);
GO

-- USE statement
USE AdventureWorks;