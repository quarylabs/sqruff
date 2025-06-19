-- Test case for AL05: Alias used in WHERE clause with IN statement
-- This tests that aliases used in complex WHERE conditions are properly detected
SELECT
    COUNT(*)
FROM
    schema1.Table_Sales_Position_Reference AS op2ref WITH(NOLOCK)
    INNER JOIN schema1.TBL_POS_DATA AS Position WITH(NOLOCK) ON Position.I_POS_ID = op2ref.i_position_id
    INNER JOIN schema1.TBL_POS_DATA AS PositionRef WITH(NOLOCK) ON PositionRef.I_POS_ID = op2ref.i_positionref_id
WHERE
    -- Alias used in IN clause
    OrderPositions.I_POS_ID IN (op2ref.i_position_id, op2ref.i_positionref_id)
    -- Alias used in equality check
    AND op2ref.i_referencetype_id = 1
    -- Alias used in IS NULL check
    AND op2ref.dt_deleted IS NULL;