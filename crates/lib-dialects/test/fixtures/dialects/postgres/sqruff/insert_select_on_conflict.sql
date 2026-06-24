INSERT INTO t (a, b) SELECT 1, 2 ON CONFLICT (a, b) DO NOTHING;

INSERT INTO t (a, b) SELECT 1, 2 ON CONFLICT (a) DO UPDATE SET b = 1;

INSERT INTO t (a, b) SELECT 1, 2 RETURNING a, b;

WITH ins AS (
    INSERT INTO link (parent_unid, child_unid)
    SELECT $1, unnest($2::uuid[])
    ON CONFLICT (parent_unid, child_unid) DO NOTHING
)
SELECT 1;
