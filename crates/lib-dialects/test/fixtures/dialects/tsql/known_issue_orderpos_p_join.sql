-- Known T-SQL Parser Issue: sao.ORDERPOS_P in JOIN clauses
-- Issue: When the table sao.ORDERPOS_P appears in a JOIN clause with an alias and WITH hint,
-- the parser fails to parse beyond the table reference, causing AL05 false positives.

-- This parses correctly (in FROM clause):
SELECT * FROM sao.ORDERPOS_P AS Position WITH(NOLOCK);

-- This fails to parse correctly (in JOIN clause):
-- Everything after "sao.ORDERPOS_P" becomes unparsable
SELECT * 
FROM dbo.anytable t
INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON t.id = Position.id;

-- The issue is specific to this exact table name pattern in JOIN contexts.
-- Other similar patterns work fine:
SELECT * 
FROM dbo.anytable t
INNER JOIN sao.OTHER_TABLE_P AS Position WITH(NOLOCK) ON t.id = Position.id;

-- This causes AL05 false positives because aliases used in unparsable sections
-- are not detected as being referenced.