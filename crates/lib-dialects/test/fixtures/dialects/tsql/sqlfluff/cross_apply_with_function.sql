-- Test case for CROSS APPLY with table-valued function
SELECT
    p.ProductID,
    p.ProductName,
    s.StockLevel
FROM
    Products p
    CROSS APPLY dbo.GetCurrentStock(p.ProductID) s
WHERE
    s.StockLevel < 10