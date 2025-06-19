-- Incremental test to isolate the issue
-- 1. Works
SELECT * FROM sao.ORDERPOS_P;

-- 2. Works
SELECT * FROM sao.ORDERPOS_P AS Position;

-- 3. Works
SELECT * FROM sao.ORDERPOS_P AS Position WITH(NOLOCK);

-- 4. Works
SELECT * FROM t1 JOIN sao.ORDERPOS_P ON t1.id = ORDERPOS_P.id;

-- 5. Works
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS p ON t1.id = p.id;

-- 6. FAILS - this specific combination
SELECT * FROM t1 JOIN sao.ORDERPOS_P AS Position ON t1.id = Position.id;