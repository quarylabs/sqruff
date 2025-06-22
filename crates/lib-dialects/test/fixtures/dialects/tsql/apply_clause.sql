SELECT
    OrderIDs.[value],
    Details.ProductName
FROM
    @OrderIDs AS OrderIDs
    OUTER APPLY (
        SELECT
            ProductName = p.Name,
            OrderID = OrderIDs.[value]
        FROM
            Products p
        WHERE
            p.OrderID = OrderIDs.[value]
    ) AS Details