-- Test case for AL02: Implicit column aliasing should use AS keyword
-- This demonstrates the T-SQL specific syntax for column aliasing
SELECT TOP (@RowLimit)
    1 AS RecordType
    ,Orders.order_id AS RecordID
    ,Orders.order_num AS RecordNumber
    ,COALESCE(Orders.description, '') AS RecordDescription
    ,Orders.status AS RecordStatus
    ,CAST(Orders.order_date AS DATETIME) AT TIME ZONE 'UTC' AS RecordDate
    ,COALESCE(Orders.total_amount, 0) AS TotalAmount
    ,(CASE Orders.is_cancelled WHEN 1 THEN -1 ELSE 1 END) AS Multiplier
    ,(
        SELECT
            '[' + STRING_AGG('"' + CAST(employee_name AS NVARCHAR(MAX)) + '"', ',') WITHIN GROUP (ORDER BY employee_role) + ']'
        FROM
            (
                SELECT
                    (Users.first_name + ' ' + Users.last_name) AS employee_name
                    ,Assignments.role_id AS employee_role
                FROM
                    dbo.order_assignments AS Assignments WITH(NOLOCK)
                    LEFT OUTER JOIN dbo.users AS Users WITH(NOLOCK) ON Users.user_id = Assignments.user_id
                WHERE
                    Assignments.order_id = Orders.order_id
            ) AS emp_data
    ) AS AssignedEmployees
    ,Orders.customer_id AS CustomerID
    ,Orders.contact_id AS ContactID
    ,Customers.company_name AS CompanyName
    ,COALESCE(Contacts.last_name, '') AS ContactLastName
    ,COALESCE(Contacts.first_name, '') AS ContactFirstName
FROM
    dbo.orders AS Orders WITH(NOLOCK)
    LEFT OUTER JOIN dbo.payment_terms AS PaymentTerms WITH(NOLOCK) ON PaymentTerms.term_id = Orders.payment_term_id
    LEFT OUTER JOIN dbo.customers AS Customers WITH(NOLOCK) ON Customers.customer_id = Orders.customer_id
    LEFT OUTER JOIN dbo.contacts AS Contacts WITH(NOLOCK) ON Contacts.contact_id = Orders.contact_id
    OUTER APPLY (
        SELECT
            SUM(COALESCE(LineItems.amount, 0)) AS TotalSales
            ,SUM(COALESCE(LineItems.cost, 0)) AS TotalCost
        FROM
            dbo.orders AS ord WITH(NOLOCK)
            LEFT OUTER JOIN dbo.order_items AS LineItems WITH(NOLOCK) ON LineItems.order_id = ord.order_id AND LineItems.item_type IN (1,2) AND LineItems.deleted_at IS NULL
        WHERE
            ord.order_id = Orders.order_id
    ) AS Summary
WHERE
    Orders.deleted_at IS NULL
ORDER BY
    Orders.order_date DESC