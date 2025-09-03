-- Test case for APPLY clause with table alias
SELECT
    c.CustomerID,
    c.CustomerName,
    Orders.TotalAmount
FROM
    Customers AS c
    OUTER APPLY (
        SELECT TotalAmount = SUM(o.Amount)
        FROM Orders AS o
        WHERE o.CustomerID = c.CustomerID
    ) AS Orders
WHERE
    c.Active = 1