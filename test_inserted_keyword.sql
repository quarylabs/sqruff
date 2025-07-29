-- Test INSERT keyword
SELECT * FROM table1
INSERT INTO table2 VALUES (1);

-- Test if "insert" prefix causes issues
MERGE t1 USING t2 ON t1.id = t2.id
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT insert.col;