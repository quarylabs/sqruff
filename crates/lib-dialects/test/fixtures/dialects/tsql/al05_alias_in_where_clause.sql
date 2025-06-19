-- Test case for AL05: Alias used in WHERE clause with IN statement
-- This tests that aliases used in complex WHERE conditions are properly detected
SELECT
    COUNT(*)
FROM
    sao.NegSoft_ERP_SalesOrderPositionReference AS op2ref WITH(NOLOCK)
    INNER JOIN sao.ORDERPOS_P AS Position WITH(NOLOCK) ON Position.I_ORDERPOS_P = op2ref.i_position_id
    INNER JOIN sao.ORDERPOS_P AS PositionRef WITH(NOLOCK) ON PositionRef.I_ORDERPOS_P = op2ref.i_positionref_id
WHERE
    -- Alias used in IN clause
    OrderPositions.I_ORDERPOS_P IN (op2ref.i_position_id, op2ref.i_positionref_id)
    -- Alias used in equality check
    AND op2ref.i_referencetype_id = 1
    -- Alias used in IS NULL check
    AND op2ref.dt_deleted IS NULL;