SELECT 
    CASE 
        WHEN 1 > = 1 THEN 'ge_spaced'
        WHEN 1 < = 1 THEN 'le_spaced' 
        WHEN 1 <   > 1 THEN 'ne_spaced'
        WHEN 1 ! = 1 THEN 'not_eq_spaced'
        WHEN 1 ! < 1 THEN 'not_lt_spaced'
        WHEN 1 !  > 1 THEN 'not_gt_spaced'
        ELSE 'default'
    END;