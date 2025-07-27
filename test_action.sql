MERGE Production.UnitMeasure AS tgt
USING source_table AS src
ON tgt.id = src.id
WHEN MATCHED THEN UPDATE SET tgt.name = src.name
OUTPUT $action, inserted.id;