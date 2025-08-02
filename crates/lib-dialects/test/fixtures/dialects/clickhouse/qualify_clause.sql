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

-- QUALIFY with nested function calls
SELECT 
    product_id,
    sales_amount,
    row_number() OVER (PARTITION BY product_id ORDER BY sales_amount DESC) as sales_rank
FROM product_sales
QUALIFY abs(sales_rank - 2) < 1;

-- QUALIFY with NULL handling
SELECT 
    customer_id,
    order_date,
    row_number() OVER (PARTITION BY customer_id ORDER BY order_date DESC) as recent_order_rank
FROM orders
QUALIFY recent_order_rank = 1 AND order_date IS NOT NULL;

-- QUALIFY with multiple window functions in expression
SELECT 
    region,
    sales_total,
    row_number() OVER (ORDER BY sales_total DESC) as rank_by_sales,
    dense_rank() OVER (ORDER BY sales_total DESC) as dense_rank_sales
FROM regional_sales
QUALIFY rank_by_sales <= 3 OR dense_rank_sales = 1;

-- QUALIFY with parentheses
SELECT 
    dept_id,
    employee_count,
    percent_rank() OVER (ORDER BY employee_count) as pct_rank
FROM departments
QUALIFY (pct_rank >= 0.75 AND employee_count > 10);