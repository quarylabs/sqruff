MERGE INTO Production.ProductInventory WITH (ROWLOCK) AS pi
USING (SELECT ProductID, SUM(OrderQty)
       FROM Sales.SalesOrderDetail AS sod
       JOIN Sales.SalesOrderHeader AS soh
       ON sod.SalesOrderID = soh.SalesOrderID
       GROUP BY ProductID) AS src (ProductID, OrderQty)
ON pi.ProductID = src.ProductID
WHEN MATCHED AND pi.Quantity - src.OrderQty >= 0
    THEN UPDATE SET pi.Quantity = pi.Quantity - src.OrderQty
WHEN MATCHED AND pi.Quantity - src.OrderQty <= 0
    THEN DELETE
WHEN NOT MATCHED BY TARGET AND src.ProductID IS NOT NULL
    THEN INSERT (ProductID, Quantity)
    VALUES (src.ProductID, src.OrderQty)
WHEN NOT MATCHED BY SOURCE
    THEN UPDATE SET pi.IsActive = 0
OUTPUT $action, Inserted.ProductID, Deleted.ProductID;