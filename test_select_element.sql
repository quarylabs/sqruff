-- Test if SelectClauseElementSegment can handle table-prefixed wildcards
SELECT deleted.* FROM table1;
SELECT inserted.* FROM table1;
SELECT t1.* FROM table1 t1;