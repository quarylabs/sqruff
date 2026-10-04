-- Wildcard on an array element (or nested field) accepts EXCEPT and REPLACE.
-- https://github.com/sqlfluff/sqlfluff/issues/8186

SELECT
    error,
    results[0].* EXCEPT (cola)
FROM `project`.`dataset`.`table`;

SELECT results[0].* REPLACE (1 AS cola)
FROM `project`.`dataset`.`table`;

SELECT s.arr[0].field.* EXCEPT (x)
FROM t;

SELECT results[0].*
FROM t;
