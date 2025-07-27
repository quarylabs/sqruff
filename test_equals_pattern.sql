-- Test if equals pattern interferes
SELECT CASE = 1;  -- This should fail as invalid SQL
SELECT StatusCode = CASE WHEN 1=1 THEN 'A' END;  -- T-SQL style alias