SELECT DISTINCT TOP 5 some_value FROM some_table;

select
    'Tabellen' as Objekt,
    Count(*) as Anzahl
from dbo.sql_modules;

SELECT PROPERTY FROM example_table;

SELECT test(default, 2);

SELECT CURRENT_USER, SESSION_USER, SYSTEM_USER, USER, test(default, 2)
FROM dbo.all_pop;

-- naked identifier with extended Unicode characters
select field1 AS 日期差多少天;
