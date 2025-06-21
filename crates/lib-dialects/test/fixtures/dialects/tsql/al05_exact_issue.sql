-- Exact reproduction of the AL05 issue
SELECT COUNT(*)
FROM schema1.Table_Sales_Position_Reference AS op2ref WITH(NOLOCK)
INNER JOIN schema1.TBL_POS_DATA AS Position WITH(NOLOCK) ON Position.I_POS_ID = op2ref.i_position_id
WHERE op2ref.i_referencetype_id = 1;