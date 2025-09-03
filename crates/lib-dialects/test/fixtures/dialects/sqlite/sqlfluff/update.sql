UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING *;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING column1;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING column1 AS c1, column2;
