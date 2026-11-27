-- OFFSET and FETCH end the SELECT clause when there is no FROM, like LIMIT.
SELECT 1 AS one OFFSET 0;
SELECT 1 AS one LIMIT 1 OFFSET 0;
SELECT 1 AS one FETCH FIRST 1 ROW ONLY;
SELECT 1 AS one OFFSET 0 FETCH NEXT 1 ROWS ONLY;

-- An empty select list followed by OFFSET, not OFFSET read as a data type.
SELECT OFFSET 0;

-- A LATERAL subquery computing one value, with OFFSET 0 so the planner
-- does not merge it into the outer query.
SELECT t.id, x.total
FROM t
CROSS JOIN LATERAL (
    SELECT (SELECT count(*) FROM u WHERE u.t_id = t.id) AS total
    OFFSET 0
) AS x;
