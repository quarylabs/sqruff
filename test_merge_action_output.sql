MERGE table1 t1
USING table2 t2 ON t1.id = t2.id  
WHEN MATCHED THEN UPDATE SET col = 1
OUTPUT $action;