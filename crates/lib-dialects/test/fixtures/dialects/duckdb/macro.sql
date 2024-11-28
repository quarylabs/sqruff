CREATE MACRO one() AS (SELECT 1);

CREATE MACRO add(a, b) AS a + b;

CREATE FUNCTION add(a, b) AS a + b;

CREATE MACRO ifelse(a, b, c) AS CASE WHEN a THEN b ELSE c END;

CREATE MACRO plus_one(a) AS (WITH cte AS (SELECT 1 AS a) SELECT cte.a + a FROM cte);

CREATE MACRO arr_append(l, e) AS list_concat(l, list_value(e));

CREATE TEMP MACRO add(a, b) AS a + b;

CREATE TEMPORARY MACRO add(a, b) AS a + b;

-- CREATE OR REPLACE MACRO add(a, b) AS a + b;
-- CREATE MACRO add_default(a, b := 5) AS a + b;
--
-- CREATE MACRO static_table() AS TABLE
--     SELECT 'Hello' AS column1, 'World' AS column2;
--
-- CREATE MACRO dynamic_table(col1_value, col2_value) AS TABLE
--     SELECT col1_value AS column1, col2_value AS column2;
--
-- CREATE OR REPLACE TEMP MACRO dynamic_table(col1_value, col2_value) AS TABLE
--     SELECT col1_value AS column1, col2_value AS column2
--     UNION ALL
--     SELECT 'Hello' AS col1_value, 456 AS col2_value;
--
-- CREATE MACRO get_users(i) AS TABLE
--     SELECT * FROM users WHERE uid IN (SELECT unnest(i));
--
-- CREATE TABLE users AS
--     SELECT *
--     FROM (VALUES (1, 'Ada'), (2, 'Bob'), (3, 'Carl'), (4, 'Dan'), (5, 'Eve')) t(uid, name);
-- SELECT * FROM get_users([1, 5]);
--
-- CREATE MACRO checksum(table_name) AS TABLE
--     SELECT bit_xor(md5_number(COLUMNS(*)::VARCHAR))
--     FROM query_table(table_name);
--
-- CREATE TABLE tbl AS SELECT unnest([42, 43]) AS x, 100 AS y;
-- SELECT * FROM checksum('tbl');
--
-- CREATE MACRO add_x
--     (a, b) AS a + b,
--     (a, b, c) AS a + b + c;