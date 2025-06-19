-- Test variations of ORDERPOS
SELECT * FROM sao.ORDERPO_P AS Position;  -- Should work
SELECT * FROM sao.ORDERPOS_P AS Position;  -- Should fail
SELECT * FROM sao.ORDERPOSS_P AS Position;  -- Test