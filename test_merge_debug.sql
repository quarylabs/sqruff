-- Test 1: Simple MERGE without OUTPUT (should work)
MERGE t1
USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1;

-- Test 2: Simple MERGE with OUTPUT (problem case)
MERGE t1
USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT $action;