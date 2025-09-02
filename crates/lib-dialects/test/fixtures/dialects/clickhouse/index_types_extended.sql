-- Test bloom_filter index with various parameter configurations
CREATE TABLE test_bloom_filter (
    id Int32,
    name String,
    user_id Int32,
    created_at DateTime64(6),
    -- Test bloom_filter with single parameter
    INDEX idx_name name TYPE bloom_filter(0.01) GRANULARITY 1,
    -- Test bloom_filter with multiple parameters  
    INDEX idx_user user_id TYPE bloom_filter(0.001, 256) GRANULARITY 2,
    -- Test bloom_filter without parameters (existing functionality)
    INDEX idx_simple name TYPE bloom_filter GRANULARITY 1
) ENGINE = Memory;

-- Test extended index types: ngrambf_v1, tokenbf_v1, hypothesis
CREATE TABLE test_extended_indexes (
    id UInt64,
    text_content String,
    search_terms String,
    vector_data Array(Float32),
    -- Test ngrambf_v1 index type (for substring search)
    INDEX idx_ngram text_content TYPE ngrambf_v1(3, 256, 3, 0) GRANULARITY 1,
    -- Test tokenbf_v1 index type (for token-based search)
    INDEX idx_token search_terms TYPE tokenbf_v1(256, 2, 0) GRANULARITY 2,
    -- Test hypothesis index type (for hypothesis testing)
    INDEX idx_hypothesis vector_data TYPE hypothesis GRANULARITY 4
) ENGINE = MergeTree()
ORDER BY id;

-- Test all index types together
CREATE TABLE test_all_index_types (
    id UInt64,
    name String,
    description String,
    tags Array(String),
    score Float64,
    -- Standard index types
    INDEX idx_minmax score TYPE minmax GRANULARITY 1,
    INDEX idx_set name TYPE set(100) GRANULARITY 1,
    INDEX idx_bloom tags TYPE bloom_filter(0.01) GRANULARITY 1,
    -- Extended index types
    INDEX idx_ngram description TYPE ngrambf_v1(3, 256, 3, 0) GRANULARITY 2,
    INDEX idx_token tags TYPE tokenbf_v1(256, 2, 0) GRANULARITY 2,
    INDEX idx_hypothesis score TYPE hypothesis GRANULARITY 1
) ENGINE = MergeTree()
ORDER BY id;