-- Test different table name patterns with Position alias
SELECT * FROM t1 JOIN sao.ORDERP AS Position ON t1.id = Position.id;
SELECT * FROM t1 JOIN sao.ORDERPO AS Position ON t1.id = Position.id;
SELECT * FROM t1 JOIN sao.ORDERPOS AS Position ON t1.id = Position.id;
SELECT * FROM t1 JOIN sao.ORDERPOS_ AS Position ON t1.id = Position.id;
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;