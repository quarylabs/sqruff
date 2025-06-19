-- Test sao.ORDERPOS_P with WITH clause in JOIN
SELECT * FROM t1
JOIN sao.ORDERPOS_P AS o WITH(NOLOCK) ON t1.id = o.id;