-- ClickHouse QUALIFY clause examples
SELECT 
    user_id,
    event_timestamp,
    row_number() OVER (PARTITION BY user_id ORDER BY event_timestamp DESC) as rn,
    count(*) OVER (PARTITION BY user_id) as user_event_count
FROM events
QUALIFY rn = 1;

-- QUALIFY with complex expression
SELECT 
    user_id,
    revenue,
    rank() OVER (PARTITION BY category ORDER BY revenue DESC) as revenue_rank
FROM sales
QUALIFY revenue_rank <= 10 AND revenue > 1000;

-- QUALIFY with multiple conditions
SELECT 
    id,
    name,
    score,
    dense_rank() OVER (ORDER BY score DESC) as rank
FROM students
QUALIFY rank BETWEEN 1 AND 5 OR score > 95;