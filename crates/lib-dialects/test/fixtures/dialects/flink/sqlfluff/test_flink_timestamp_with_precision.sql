CREATE TABLE my_table (
    id INT,
    event_time TIMESTAMP(3),
    processing_time TIMESTAMP_LTZ(3)
) WITH (
    'connector' = 'kafka'
);
