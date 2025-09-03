SELECT
    CAST(
        JSON_PARSE(table_name.column_name) AS MAP<VARCHAR, VARCHAR>
    ) AS json_map
FROM table_name;

CREATE TABLE map_table(c1 map<string, integer>) LOCATION '...';
INSERT INTO map_table values(MAP(ARRAY['foo', 'bar'], ARRAY[1, 2]));
