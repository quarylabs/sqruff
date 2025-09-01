-- Simple ternary expression
SELECT 1 = 1 ? 'yes' : 'no' AS result;

-- Ternary with parentheses and AND
SELECT (true ? 1 : 0) AND (false ? 1 : 0) AS ternary_with_and;

-- Ternary with comparison
SELECT x > 5 ? 'greater' : 'not greater' AS comparison_result;

-- Ternary in WHERE with parentheses  
SELECT * FROM users WHERE (age >= 18 ? 1 : 0) = 1;

-- Ternary with arithmetic
SELECT 2 + 3 > 4 ? 'yes' : 'no' AS arithmetic_test;

-- Nested ternary
SELECT 
    score >= 90 ? 'A' : 
    (score >= 80 ? 'B' : 
    (score >= 70 ? 'C' : 'F')) AS grade
FROM students;