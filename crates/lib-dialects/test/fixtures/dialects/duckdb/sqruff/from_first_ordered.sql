FROM stream.stream
WHERE ($start IS NULL OR create_at >= $start)
ORDER BY channel
LIMIT $limit OFFSET $offset;

FROM stream.stream
SELECT channel
WHERE channel == $channel
ORDER BY channel DESC;

WITH src AS (FROM stream.stream ORDER BY channel LIMIT $limit)
FROM src ORDER BY channel;

FROM table_a UNION ALL FROM table_b ORDER BY channel;
