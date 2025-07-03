-- Test SQL Server 2017+ identifiers (# at end)

-- Regular identifiers with # at end
SELECT * FROM orders#;

-- Qualified identifiers with # at end
SELECT o#.id# FROM orders# o#;