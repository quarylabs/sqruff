SELECT table1.col
FROM table1
LEFT LOOP JOIN table2
    ON table1.col = table2.col;
