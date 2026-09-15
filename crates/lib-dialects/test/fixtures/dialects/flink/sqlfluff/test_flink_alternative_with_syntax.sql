CREATE TABLE test_table (
  data_info ROW<`info` STRING>,
  name STRING,
  score DOUBLE,
  total_count DOUBLE,
  active_count DOUBLE,
  metadata ROW<`details` STRING>,
  change_rate DOUBLE,
  volume DOUBLE,
  change_percentage DOUBLE,
  updated_at TIMESTAMP(3),
  category STRING
) WITH (
  connector == 'test-connector',
  environment == 'development'
);
