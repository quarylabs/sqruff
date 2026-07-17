SELECT * EXCEPT (c1) from t1;
SELECT * EXCEPT (c1, c2) from t1;

SELECT field_1
FROM table_1
EXCEPT ALL (
    SELECT field_1
    FROM table_2
);
