-- SQL Server 2017+ allows # at the end of identifiers
-- This test validates that the T-SQL dialect correctly parses these identifiers

-- Simple SELECT with # identifiers
SELECT orders#.id# FROM orders#;

-- Table alias with #
SELECT o#.total# FROM orders# AS o#;

-- Column alias with #
SELECT total AS amount# FROM orders#;