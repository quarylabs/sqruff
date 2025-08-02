-- ClickHouse higher-order functions with two sets of parentheses
SELECT 
    quantileExact(0.5)(response_time) as median_response,
    quantileExactArrayIf(0.95)(response_times, response_times > 0) as p95_response,
    avgArrayIf(response_times, response_times > 0) as avg_response,
    arraySort(x -> -x)(values) as sorted_desc,
    arrayMap(x -> x * 2)(numbers) as doubled,
    -- Regular functions should still work
    count(*) as total_count,
    sum(amount) as total_amount
FROM test_table;