MERGE t1 AS tgt
USING (SELECT 1, 2) as src (a, b)
ON tgt.id = src.a
WHEN MATCHED THEN
    UPDATE SET val = src.b
OUTPUT deleted.*, $action;