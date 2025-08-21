-- Simple select to test AggregateFunction parsing in context
SELECT CAST(NULL AS AggregateFunction(sum, UInt64)) AS agg_col;