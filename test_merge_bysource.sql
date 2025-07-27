MERGE target_table AS t
USING source_table AS s
ON t.id = s.id
WHEN NOT MATCHED BY TARGET THEN
    INSERT (id, name) VALUES (s.id, s.name)
WHEN NOT MATCHED BY SOURCE THEN
    UPDATE SET is_deleted = 1;