-- Test nested JOIN structures in T-SQL
SELECT 1 AS RegionCode
FROM BA
LEFT OUTER JOIN I
    LEFT OUTER JOIN P
        ON I.Pcd = P.Iid
    ON BA.Iid = I.Bcd;

-- Test complex nested JOINs
SELECT *
FROM Orders o
LEFT OUTER JOIN OrderDetails od
    INNER JOIN Products p
        ON od.ProductID = p.ProductID
    ON o.OrderID = od.OrderID
LEFT OUTER JOIN Customers c
    ON o.CustomerID = c.CustomerID;

-- Test nested JOINs with T-SQL algorithm hints
SELECT *
FROM table1 t1
LEFT OUTER HASH JOIN table2 t2
    INNER MERGE JOIN table3 t3
        ON t2.id = t3.t2_id
    ON t1.id = t2.t1_id
FULL OUTER LOOP JOIN table4 t4
    ON t1.id = t4.t1_id;