-- Exact reproduction of the AL05 issue
SELECT COUNT(*)
FROM sao.NegSoft_ERP_SalesOrderPositionReference AS op2ref WITH(NOLOCK)
INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON Position.I_ORDERPOS_P = op2ref.i_position_id
WHERE op2ref.i_referencetype_id = 1;