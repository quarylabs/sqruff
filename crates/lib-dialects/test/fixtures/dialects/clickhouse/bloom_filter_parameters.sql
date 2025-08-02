CREATE TABLE test_bloom_filter (
    id Int32,
    name String,
    user_id Int32,
    created_at DateTime64(6),
    -- Test bloom_filter with single parameter
    INDEX idx_name name TYPE bloom_filter (0.01) GRANULARITY 1,
    -- Test bloom_filter with multiple parameters  
    INDEX idx_user user_id TYPE bloom_filter (0.001, 256) GRANULARITY 2,
    -- Test bloom_filter without parameters (existing functionality)
    INDEX idx_simple name TYPE bloom_filter GRANULARITY 1
) ENGINE = Memory;

-- Test complex table with multiple parameterized indexes and projections
CREATE TABLE analytics_events (
    user_id Int32,
    event_type String,
    timestamp DateTime64(6),
    properties String,
    -- Multiple bloom_filter indexes with parameters
    INDEX idx_user_id user_id TYPE bloom_filter (0.01) GRANULARITY 1,
    INDEX idx_event_type event_type TYPE bloom_filter (0.001) GRANULARITY 1,
    INDEX idx_timestamp timestamp TYPE minmax GRANULARITY 4,
    -- Projection with complex SELECT
    PROJECTION user_events (
        SELECT 
            user_id,
            event_type,
            count() as event_count,
            max(timestamp) as last_event
        GROUP BY user_id, event_type
        ORDER BY user_id, event_count DESC
    )
) ENGINE = ReplacingMergeTree
PARTITION BY toYYYYMM(timestamp)
PRIMARY KEY user_id
ORDER BY (user_id, timestamp);