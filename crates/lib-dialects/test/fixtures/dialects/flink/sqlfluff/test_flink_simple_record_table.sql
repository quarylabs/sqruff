CREATE TABLE table3 (
  service STRING,
  type STRING,
  from_id STRING,
  to_id STRING,
  amount DOUBLE,
  quantity DOUBLE,
  executed_at TIMESTAMP(3),
  id STRING,
  request_url STRING,
  direction STRING,
  client_received_timestamp TIMESTAMP(3),
  job_timestamp TIMESTAMP(3),
  job_id STRING,
  processor STRING
) WITH (
  'connector' = 'test-connector',
  'project' = 'test-project',
  'dataset' = 'test-dataset',
  'table' = 'test-records'
);
