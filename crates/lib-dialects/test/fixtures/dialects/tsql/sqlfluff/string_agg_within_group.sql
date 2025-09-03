-- T-SQL STRING_AGG with WITHIN GROUP clause tests

-- Basic STRING_AGG with WITHIN GROUP
SELECT STRING_AGG(FirstName, ', ') WITHIN GROUP (ORDER BY FirstName)
FROM Employees;

-- STRING_AGG with WITHIN GROUP and DESC order
SELECT STRING_AGG(ProductName, ' | ') WITHIN GROUP (ORDER BY Price DESC)
FROM Products;

-- GROUP BY with STRING_AGG and WITHIN GROUP
SELECT 
    Department,
    STRING_AGG(EmployeeName, ', ') WITHIN GROUP (ORDER BY HireDate)
FROM Employees
GROUP BY Department;

-- Multiple aggregations with WITHIN GROUP
SELECT 
    CategoryID,
    STRING_AGG(ProductName, ', ') WITHIN GROUP (ORDER BY ProductName),
    COUNT(*) as ProductCount
FROM Products
GROUP BY CategoryID;

-- STRING_AGG with complex ORDER BY in WITHIN GROUP
SELECT 
    Region,
    STRING_AGG(City, ', ') WITHIN GROUP (ORDER BY Population DESC, City ASC)
FROM Cities
GROUP BY Region;

-- STRING_AGG with CAST in WITHIN GROUP
SELECT STRING_AGG(CAST(ProductID AS VARCHAR(10)), '-') WITHIN GROUP (ORDER BY ProductID)
FROM Products
WHERE CategoryID = 1;

-- Nested in subquery
SELECT *
FROM (
    SELECT 
        DepartmentID,
        STRING_AGG(EmployeeName, ', ') WITHIN GROUP (ORDER BY Salary DESC) as TopEarners
    FROM Employees
    GROUP BY DepartmentID
) AS DeptSummary;