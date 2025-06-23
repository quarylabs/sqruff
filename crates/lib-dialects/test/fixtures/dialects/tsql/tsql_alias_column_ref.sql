-- Test T-SQL alias equals syntax with column references
SELECT
    -- Simple identifier
    SimpleAlias = value,
    -- Column reference with table prefix
    QualifiedAlias = table1.column1,
    -- Column reference with schema.table prefix
    FullyQualifiedAlias = dbo.table1.column1,
    -- Function call
    FunctionAlias = SUM(quantity),
    -- Complex expression
    ExpressionAlias = table1.price * table1.quantity
FROM table1;