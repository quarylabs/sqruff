IF 1 = 1
    SELECT col1, col2, 'This is a long string'
        AS alias_name
    FROM table1
ELSE
    SELECT col3 FROM table2