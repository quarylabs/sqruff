-- Test 1: IF outside procedure (should work)
IF 1 = 1
BEGIN
    SELECT 1
END

-- Test 2: Simple procedure without IF (should work)
CREATE PROCEDURE test1
AS
SELECT 1

-- Test 3: Procedure with IF (problematic?)
CREATE PROCEDURE test2
AS
IF 1 = 1
BEGIN
    SELECT 1
END