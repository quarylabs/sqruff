SELECT
  name,
  ROW_NUMBER() OVER(PARTITION BY dept),
  salary as sal
FROM employees;

SELECT c, a, b, group_concat(b, '.') FILTER (WHERE c!='two') OVER (
  ORDER BY a
) AS group_concat
FROM t1 ORDER BY a;
