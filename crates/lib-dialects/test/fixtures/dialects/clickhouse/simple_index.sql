CREATE TABLE users (
    id UInt64,
    name String,
    INDEX idx_name name TYPE minmax
) ENGINE = MergeTree()
ORDER BY id;