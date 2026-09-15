CREATE TABLE my_table (
    id INT,
    name STRING,
    full_name AS CONCAT(name, '_suffix')
) WITH (
    'connector' = 'kafka'
);
