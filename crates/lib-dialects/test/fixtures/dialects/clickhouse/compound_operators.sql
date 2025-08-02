-- Test all compound comparison operators
SELECT *
FROM test_table
WHERE col1 >= 100
  AND col2 <= 200
  AND col3 != 'invalid'
  AND col4 <> 'excluded'
  AND col5 > 50
  AND col6 < 150;