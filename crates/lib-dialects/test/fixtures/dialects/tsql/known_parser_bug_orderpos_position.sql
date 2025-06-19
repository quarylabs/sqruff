-- Known T-SQL Parser Bug: sao.ORDERPOS_P AS Position in JOIN
-- 
-- Issue: The T-SQL parser fails when the specific table "sao.ORDERPOS_P" 
-- is used in a JOIN clause with the alias "Position". This causes the 
-- rest of the query to become unparsable, leading to AL05 false positives.
--
-- Root cause: Likely a parser ambiguity with the identifier sequence
-- "ORDERPOS_P AS Position" in JOIN context.

-- These all work correctly:
SELECT * FROM sao.ORDERPOS_P AS Position;  -- Works in FROM
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS p ON t1.id = p.id;  -- Works with different alias
SELECT * FROM t1 JOIN sao.OTHER_TABLE AS Position ON t1.id = Position.id;  -- Works with different table

-- This specific combination FAILS:
SELECT * FROM t1 
JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;

-- Also fails with WITH clause:
SELECT * FROM t1 
JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON t1.id = Position.id;

-- Workaround: Use a different alias name
SELECT * FROM t1 
JOIN sao.ORDERPOS_P AS pos WITH(NOLOCK) ON t1.id = pos.id;