UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING *;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING column1;

UPDATE table_name SET column1 = value1, column2 = value2 WHERE a=1 RETURNING column1 AS c1, column2;

UPDATE OR IGNORE table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE OR ABORT table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE OR FAIL table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE OR REPLACE table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE OR ROLLBACK table_name SET column1 = value1, column2 = value2 WHERE a=1;

UPDATE table_name SET (context, country) = (SELECT context, country FROM Nations);
