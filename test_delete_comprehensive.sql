-- Test 1: Simple DELETE with FROM
DELETE FROM Sales.SalesPersonQuotaHistory;
GO

-- Test 2: DELETE with FROM and WHERE
DELETE FROM Production.ProductCostHistory
WHERE StandardCost > 1000.00;
GO

-- Test 3: DELETE without FROM keyword (T-SQL allows this)
DELETE Production.ProductCostHistory
WHERE StandardCost BETWEEN 12.00 AND 14.00
      AND EndDate IS NULL;
PRINT 'Number of rows deleted is ' + CAST(@@ROWCOUNT as char(3));
GO