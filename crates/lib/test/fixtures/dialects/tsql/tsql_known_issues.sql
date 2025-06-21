-- T-SQL Variable Parsing Tests
-- All these tests now parse correctly after the fix

-- Test 1: INSERT with variables in VALUES
INSERT INTO table1 (col1, col2) VALUES (@var1, @var2);

-- Test 2: WHERE clause with variable in parentheses
SELECT * FROM table1 WHERE (@param = 0 OR col1 = @param);

-- Test 3: Variable after comment in parentheses
SELECT * FROM table1 WHERE ( -- filter
    col1 = @param
);

-- Test 4: BETWEEN with variables in parentheses
SELECT * FROM table1 WHERE (
    date_col BETWEEN @StartDate AND @EndDate
);

-- Test 5: Variables in function calls
SELECT CONCAT(@prefix, 'suffix', @suffix);

-- Test 6: Nested parentheses with variables
SELECT * FROM table1 WHERE (col1 = @val1 AND (col2 = @val2 OR col3 = @val3));

-- Basic variable usage (these also work):
SELECT * FROM table1 WHERE col1 = @param;
SELECT @var1, @var2;
DECLARE @param INT = 123;