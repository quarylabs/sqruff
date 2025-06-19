-- Workaround for AL05 false positive with sao.ORDERPOS_P in JOIN
-- The parser fails with certain table patterns in JOIN clauses

-- Original problematic query (alias op2ref reported as unused):
/*
SELECT COUNT(*)
FROM sao.NegSoft_ERP_SalesOrderPositionReference AS op2ref WITH(NOLOCK)
INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON Position.I_ORDERPOS_P = op2ref.i_position_id
WHERE op2ref.i_referencetype_id = 1;
*/

-- Workaround 1: Use square brackets for the problematic table
SELECT COUNT(*)
FROM sao.NegSoft_ERP_SalesOrderPositionReference AS op2ref WITH(NOLOCK)
INNER JOIN sao.[ORDERPOS_P] AS Position WITH(NOLOCK) ON Position.I_ORDERPOS_P = op2ref.i_position_id
WHERE op2ref.i_referencetype_id = 1;

-- Workaround 2: Use table hint before alias
SELECT COUNT(*)
FROM sao.NegSoft_ERP_SalesOrderPositionReference AS op2ref WITH(NOLOCK)
INNER JOIN sao.ORDERPOS_P WITH(NOLOCK) AS Position ON Position.I_ORDERPOS_P = op2ref.i_position_id
WHERE op2ref.i_referencetype_id = 1;