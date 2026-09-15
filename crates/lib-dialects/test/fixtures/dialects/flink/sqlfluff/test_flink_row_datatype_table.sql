CREATE TABLE table1 (
  data_info ROW<`name` STRING>,
  email STRING,
  score DOUBLE,
  total_points DOUBLE,
  active_points DOUBLE,
  metadata ROW<`description` STRING>,
  change_percentage DOUBLE,
  volume DOUBLE,
  rate_change_percentage DOUBLE,
  last_updated TIMESTAMP(3),
  status STRING
)  WITH (
  'connector' = 'test-connector',
  'project' = 'test-project',
  'dataset' = 'test-dataset'
);
