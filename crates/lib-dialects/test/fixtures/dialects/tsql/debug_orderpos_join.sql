-- Debug ORDERPOS_P parsing in JOIN
-- This works:
SELECT * FROM sao.ORDERPOS_P AS Position WITH(NOLOCK);

-- This fails:
SELECT * FROM dbo.sometable t
INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON t.id = Position.id;