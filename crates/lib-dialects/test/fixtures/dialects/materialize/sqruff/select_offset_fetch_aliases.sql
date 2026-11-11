-- OFFSET and FETCH end the SELECT clause when there is no FROM, like LIMIT.
SELECT 1 AS one OFFSET 0;
SELECT 1 AS one FETCH FIRST 1 ROW ONLY;

-- After AS they are an alias, which Materialize accepts, not the end of the
-- SELECT clause.
SELECT 1 AS offset FROM t;
SELECT 1 AS fetch FROM t;
SELECT array_agg(x) FILTER (WHERE y) AS offset FROM t;
