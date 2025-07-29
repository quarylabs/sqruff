MERGE t1 AS tgt
USING (SELECT 1, 2) as src (a, b)
ON tgt.id = src.a
WHEN NOT MATCHED THEN
    INSERT (id, val)
    VALUES (src.a, src.b)
OUTPUT deleted.*, $action;