CREATE TABLE my_table (
    id INT,
    nested_data ROW<name STRING, age INT>
) WITH (
    'connector' = 'kafka'
);
