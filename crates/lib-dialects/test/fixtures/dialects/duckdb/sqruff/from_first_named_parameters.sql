FROM Stream.Stream
WHERE
    (
        ($create_at_start IS NULL)
        OR (create_at >= $create_at_start)
    )
    AND (($create_at IS NULL) OR (create_at == $create_at))
ORDER BY channel;

FROM Stream.Stream
SELECT channel
WHERE $stream_id IS NULL;

SELECT
    CASE
        WHEN $target == 'stream_id' THEN stream_id
        WHEN $target == 'name' THEN name
    END AS mapping
FROM Stream.Mapping;
