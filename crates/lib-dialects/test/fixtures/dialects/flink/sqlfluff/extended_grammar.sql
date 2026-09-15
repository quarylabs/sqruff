CREATE TABLE nested_types (
    id INT,
    items ARRAY<ROW<name STRING, score DECIMAL(10, 2)>>,
    attributes MAP<STRING, ARRAY<INT>>,
    tags MULTISET<STRING>,
    detail ROW(name STRING COMMENT 'field', count BIGINT),
    offset_value BIGINT METADATA FROM 'offset' VIRTUAL COMMENT 'metadata',
    computed AS id + 1 COMMENT 'computed',
    CONSTRAINT pk PRIMARY KEY (id) NOT ENFORCED
) COMMENT 'table'
PARTITIONED BY (id)
DISTRIBUTED BY HASH (id) INTO 4 BUCKETS
WITH ('connector' = 'kafka', environment == 'test')
LIKE source_table (INCLUDING ALL, EXCLUDING CONSTRAINTS, OVERWRITING OPTIONS);
CREATE TABLE ranged (id INT) DISTRIBUTED BY RANGE (id);
CREATE TABLE bucketed (id INT) DISTRIBUTED INTO 3 BUCKETS;
CREATE TABLE plain_distribution (id INT) DISTRIBUTED BY (id);
CREATE OR REPLACE TABLE copied AS (SELECT * FROM source_table);
CREATE TABLE copied_like LIKE source_table;
SELECT 1.5F, 2BD, 3e2D, 'it\'s', `escaped``name` FROM `source`;
EXPLAIN PLAN FOR SELECT * FROM source_table;
