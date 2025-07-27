MERGE Production.UnitMeasure AS tgt
USING (SELECT @UnitMeasureCode, @Name) as src (UnitMeasureCode, Name)
ON (tgt.UnitMeasureCode = src.UnitMeasureCode)
WHEN MATCHED THEN
    UPDATE SET Name = src.Name
WHEN NOT MATCHED THEN
    INSERT (UnitMeasureCode, Name)
    VALUES (src.UnitMeasureCode, src.Name);