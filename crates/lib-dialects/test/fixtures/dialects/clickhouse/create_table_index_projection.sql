CREATE TABLE users (
    id UInt64,
    name String,
    email String,
    age UInt8,
    created_at DateTime,
    INDEX idx_email email TYPE minmax GRANULARITY 1,
    INDEX idx_age age TYPE set(10) GRANULARITY 2,
    PROJECTION projection_by_age (SELECT age, count() GROUP BY age)
) ENGINE = MergeTree()
ORDER BY id;

CREATE TABLE analytics (
    user_id UInt64,
    event_name String,
    timestamp DateTime,
    properties String,
    INDEX idx_events event_name TYPE bloom_filter GRANULARITY 1,
    INDEX idx_timestamp timestamp TYPE minmax,
    PROJECTION monthly_stats (
        SELECT 
            toYYYYMM(timestamp) as month,
            event_name,
            count() as cnt
        GROUP BY month, event_name
    )
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (user_id, timestamp);