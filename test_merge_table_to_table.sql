MERGE t1
USING t2
ON t1.id = t2.id
WHEN MATCHED THEN
    UPDATE SET val = t2.val
OUTPUT deleted.*, $action;
