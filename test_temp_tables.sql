-- Test temp table names still work
SELECT * FROM #temp;
SELECT * FROM ##global;
CREATE TABLE #local (id int);
CREATE TABLE ##global (id int);

-- Test identifiers with # at end
SELECT col# FROM table1;

-- Test keywords are recognized
SELECT CASE WHEN 1=1 THEN 'yes' END;