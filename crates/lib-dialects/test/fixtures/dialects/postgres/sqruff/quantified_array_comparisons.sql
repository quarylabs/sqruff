-- Quantified comparisons accept array columns and expressions.
SELECT 1 FROM t WHERE t.x = ANY (t.arr);
SELECT 1 FROM t WHERE t.x = ANY (ARRAY[1, 2]);
SELECT 1 FROM t WHERE t.x = any(t.arr);
SELECT 1 FROM t WHERE t.x <> ALL (t.arr);
SELECT 1 FROM t WHERE t.x > ALL (ARRAY[1, 2]);
SELECT 1 FROM t WHERE t.x = SOME (t.arr);
SELECT 1 FROM t WHERE t.x = SOME (ARRAY[1, 2]);

-- Subquery comparisons must continue to parse.
SELECT 1 FROM t WHERE t.x = ANY (SELECT x FROM t2);
SELECT 1 FROM t WHERE t.x > ALL (SELECT x FROM t2);
SELECT 1 FROM t WHERE t.x = SOME (SELECT x FROM t2);

-- pg_dump uses ANY with typed arrays in schema predicates.
CREATE INDEX i ON t (x) WHERE (status = ANY (ARRAY['a'::text, 'b'::text]));
CREATE TABLE t (r text, CONSTRAINT k CHECK ((r = ANY (ARRAY['a'::text]))));
