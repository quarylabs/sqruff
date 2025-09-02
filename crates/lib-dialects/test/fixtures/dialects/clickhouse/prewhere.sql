-- PREWHERE clause
SELECT * FROM users PREWHERE age > 18;

-- PREWHERE with simple expression
SELECT * FROM users PREWHERE status = 'active';

-- PREWHERE with expression
SELECT * FROM users PREWHERE (age > 18 AND status = 'active');