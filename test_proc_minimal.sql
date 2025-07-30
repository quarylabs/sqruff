-- Test 1: Simple procedure with single statement (should work)
CREATE PROCEDURE test1
AS
SELECT 1;

-- Test 2: Procedure with BEGIN END (should work)
CREATE PROCEDURE test2
AS
BEGIN
    SELECT 1;
END;

-- Test 3: Procedure with IF (problematic?)
CREATE PROCEDURE test3
AS
IF 1 = 1
    SELECT 1;