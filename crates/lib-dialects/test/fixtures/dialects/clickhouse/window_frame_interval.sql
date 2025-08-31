-- Test RANGE BETWEEN with INTERVAL for ClickHouse window functions
SELECT
    user_id,
    sum(amount) OVER (
        PARTITION BY user_id
        ORDER BY date_col
        RANGE BETWEEN INTERVAL 28 DAY PRECEDING AND INTERVAL 1 DAY PRECEDING
    ) AS rolling_sum_28d,
    avg(amount) OVER (
        PARTITION BY user_id
        ORDER BY timestamp_col
        RANGE BETWEEN INTERVAL 7 DAY PRECEDING AND CURRENT ROW
    ) AS rolling_avg_7d
FROM transactions;

-- Test with different interval units
SELECT
    count(*) OVER (
        PARTITION BY customer_id
        ORDER BY order_date
        RANGE BETWEEN INTERVAL 3 MONTH PRECEDING AND CURRENT ROW
    ) AS orders_3m,
    sum(amount) OVER (
        PARTITION BY customer_id
        ORDER BY order_date
        RANGE BETWEEN INTERVAL 1 YEAR PRECEDING AND INTERVAL 1 MONTH PRECEDING
    ) AS revenue_year_exclude_current_month
FROM orders;