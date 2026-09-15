CREATE TABLE my_table (
    id INT,
    name STRING,
    kafka_offset BIGINT METADATA FROM 'offset'
) WITH (
    'connector' = 'kafka'
);
