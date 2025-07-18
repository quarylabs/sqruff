-- Test MERGE as join hint vs MERGE statement

-- This should parse as a JOIN with MERGE hint
SELECT * FROM A INNER MERGE JOIN B ON A.id = B.id;

-- This should parse as a MERGE statement
MERGE TableA AS target
USING TableB AS source
ON target.id = source.id
WHEN MATCHED THEN UPDATE SET target.name = source.name;