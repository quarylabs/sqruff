-- Test NakedIdentifier vs CASE keyword
SELECT 
    -- This should parse CASE as identifier (if not reserved)
    CASE,
    -- This should parse CASE as CASE expression
    CASE WHEN 1=1 THEN 'A' END,
    -- This should work (regular identifier)
    MyColumn