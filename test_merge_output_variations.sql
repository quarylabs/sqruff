-- Test 1: Simple OUTPUT with single column
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT $action;

-- Test 2: OUTPUT with inserted prefix
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT inserted.col;

-- Test 3: OUTPUT with deleted prefix
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT deleted.col;

-- Test 4: OUTPUT with star
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT deleted.*;

-- Test 5: OUTPUT with multiple items
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT deleted.*, $action, inserted.*;