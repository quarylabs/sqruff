MERGE t1
USING (SELECT 1, 2) as src (a, b)
ON t1.id = src.a
WHEN MATCHED THEN
    UPDATE SET val = src.b
OUTPUT $action;
