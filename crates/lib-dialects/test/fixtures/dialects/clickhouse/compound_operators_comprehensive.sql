-- Comprehensive test for compound operators as single tokens
SELECT id, value, status
FROM test_table
WHERE value >= 100
  AND score <= 50
  AND status != 'inactive'
  AND category <> 'deleted'
  AND created_at >= '2024-01-01'
  AND modified_at <= CURRENT_TIMESTAMP();