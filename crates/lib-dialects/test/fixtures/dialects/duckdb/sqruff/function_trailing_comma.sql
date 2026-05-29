SELECT
    list_value(1, 2, 3,) AS nums,
    COALESCE(a, b, c,) AS first_non_null
FROM my_table;
