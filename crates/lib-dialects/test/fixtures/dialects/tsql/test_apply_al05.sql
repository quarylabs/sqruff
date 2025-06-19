SELECT
    OrderStates.*
FROM
    @OrderIDs AS OrderIDs
    OUTER APPLY(
        SELECT
            OrderID = OrderIDs.value
            ,OrderProcessStateName = 'test'
        FROM
            sao.NegSoft_ERP_SalesOrderState AS NxOrderState
    ) AS OrderStates;