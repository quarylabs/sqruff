MERGE target USING source ON target.id = source.id WHEN MATCHED THEN UPDATE SET col = 1;
