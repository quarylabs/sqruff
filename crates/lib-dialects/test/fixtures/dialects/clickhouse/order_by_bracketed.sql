-- Test ORDER BY with bracketed expressions containing DESC/ASC
SELECT * FROM users ORDER BY (id, name DESC);

SELECT * FROM events ORDER BY (timestamp DESC, user_id);

SELECT * FROM logs ORDER BY (date, priority ASC, id DESC);

-- Test with NULLS and WITH FILL
SELECT * FROM metrics ORDER BY (value DESC NULLS LAST, timestamp);

-- Complex example with multiple features
SELECT user_id, count() as cnt
FROM events
GROUP BY user_id
ORDER BY (user_id, cnt DESC)
LIMIT 10;

-- Test in CREATE TABLE PROJECTION context
CREATE TABLE test_projection (
    user_id Int32,
    event_type String,
    timestamp DateTime,
    PROJECTION user_events (
        SELECT 
            user_id,
            event_type,
            count() as event_count
        GROUP BY user_id, event_type
        ORDER BY (user_id, event_count DESC)
    )
) ENGINE = MergeTree
ORDER BY (user_id, timestamp);