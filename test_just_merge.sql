-- Test just MERGE keyword in different contexts

-- This should fail to parse
LEFT MERGE JOIN TableB;

-- This should parse as MERGE statement
MERGE TableA;