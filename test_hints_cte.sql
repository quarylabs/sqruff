WITH cte (CustomerID, PersonID, StoreID) AS
(
    SELECT CustomerID, PersonID, StoreID
    FROM Sales.Customer
    WHERE PersonID IS NOT NULL
  UNION ALL
    SELECT cte.CustomerID, cte.PersonID, cte.StoreID
    FROM cte
    JOIN  Sales.Customer AS e
        ON cte.PersonID = e.CustomerID
)
SELECT CustomerID, PersonID, StoreID
FROM cte
OPTION (MAXRECURSION 2);