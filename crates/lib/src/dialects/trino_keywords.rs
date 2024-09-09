pub const RESERVED_WORDS: &str = r#"ALTER
AND
AS
BETWEEN
BY
CASE
CAST
CONSTRAINT
CREATE
CROSS
CUBE
CURRENT_CATALOG
CURRENT_DATE
CURRENT_PATH
CURRENT_ROLE
CURRENT_SCHEMA
CURRENT_TIME
CURRENT_TIMESTAMP
CURRENT_USER
DEALLOCATE
DELETE
DESCRIBE
DISTINCT
DROP
ELSE
END
ESCAPE
EXCEPT
EXECUTE
EXISTS
EXTRACT
FALSE
FOR
FROM
FULL
GROUP
GROUPING
HAVING
IN
INNER
INSERT
INTERSECT
INTO
IS
JOIN
JSON_ARRAY
JSON_EXISTS
JSON_OBJECT
JSON_QUERY
JSON_TABLE
JSON_VALUE
LEFT
LIKE
LISTAGG
LOCALTIME
LOCALTIMESTAMP
NATURAL
NORMALIZE
NOT
NULL
ON
OR
ORDER
OUTER
PREPARE
RECURSIVE
RIGHT
ROLLUP
SELECT
SKIP
TABLE
THEN
TRIM
TRUE
UESCAPE
UNION
UNNEST
USING
VALUES
WHEN
WHERE
WITH
"#;

pub const UNRESERVED_WORDS: &str = r#"ABSENT
ADD
ADMIN
AFTER
ALL
ANALYZE
ANY
ARRAY
ASC
AT
AUTHORIZATION
BERNOULLI
BIGINT
BOOLEAN
BOTH
CALL
CASCADE
CATALOG
CATALOGS
CHAR
COLUMN
COLUMNS
COMMENT
COMMIT
COMMITTED
CONDITIONAL
COPARTITION
COUNT
CURRENT
DATA
DATE
DAY
DECIMAL
DEFAULT
DEFINE
DEFINER
DENY
DESC
DESCRIPTOR
DISTRIBUTED
DOUBLE
EMPTY
ENCODING
ERROR
EXCLUDING
EXPLAIN
FETCH
FILTER
FINAL
FIRST
FOLLOWING
FORMAT
FUNCTIONS
GRACE
GRANT
GRANTED
GRANTS
GRAPHVIZ
GROUPS
HOUR
IF
IGNORE
IMMEDIATE
INCLUDING
INITIAL
INPUT
INT
INTEGER
INTERVAL
INVOKER
IO
IPADDRESS
ISOLATION
JSON
KEEP
KEY
KEYS
LAST
LATERAL
LEADING
LEVEL
LIMIT
LOCAL
LOGICAL
MAP
MATCH
MATCHED
MATCHES
MATCH_RECOGNIZE
MATERIALIZED
MEASURES
MERGE
MINUTE
MONTH
NESTED
NEXT
NFC
NFD
NFKC
NFKD
NO
NONE
NULLIF
NULLS
OBJECT
OF
OFFSET
OMIT
ONE
ONLY
OPTION
ORDINALITY
OUTPUT
OVER
OVERFLOW
PARTITION
PARTITIONS
PASSING
PAST
PATH
PATTERN
PER
PERIOD
PERMUTE
PLAN
POSITION
PRECEDING
PRECISION
PRIVILEGES
PROPERTIES
PRUNE
QUOTES
RANGE
READ
REAL
REFRESH
RENAME
REPEATABLE
REPLACE
RESET
RESPECT
RESTRICT
RETURNING
REVOKE
ROLE
ROLES
ROLLBACK
ROW
ROWS
RUNNING
SCALAR
SCHEMA
SCHEMAS
SECOND
SECURITY
SEEK
SERIALIZABLE
SESSION
SET
SETS
SHOW
SMALLINT
SOME
START
STATS
SUBSET
SUBSTRING
SYSTEM
TABLES
TABLESAMPLE
TEXT
TEXT_STRING
TIES
TIME
TIMESTAMP
TINYINT
TO
TRAILING
TRANSACTION
TRUNCATE
TRY_CAST
TYPE
UNBOUNDED
UNCOMMITTED
UNCONDITIONAL
UNIQUE
UNKNOWN
UNMATCHED
UPDATE
USE
USER
UTF16
UTF32
UTF8
UUID
VALIDATE
VALUE
VARBINARY
VARCHAR
VERBOSE
VERSION
VIEW
WINDOW
WITHIN
WITHOUT
WORK
WRAPPER
WRITE
YEAR
ZONE
"#;
