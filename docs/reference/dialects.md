# Dialects

Sqruff currently supports the following SQL dialects:

## Dialects Index

- [ansi](#ansi)
- [athena](#athena)
- [bigquery](#bigquery)
- [clickhouse](#clickhouse)
- [databricks](#databricks)
- [db2](#db2)
- [duckdb](#duckdb)
- [exasol](#exasol)
- [greenplum](#greenplum)
- [hive](#hive)
- [mariadb](#mariadb)
- [materialize](#materialize)
- [mysql](#mysql)
- [oracle](#oracle)
- [postgres](#postgres)
- [redshift](#redshift)
- [snowflake](#snowflake)
- [sparksql](#sparksql)
- [sqlite](#sqlite)
- [starrocks](#starrocks)
- [teradata](#teradata)
- [trino](#trino)
- [tsql](#tsql)
- [vertica](#vertica)

## Details

### ansi

Standard SQL syntax. The default dialect and base for all others.

**Default Casing:** `UPPERCASE`

**Quotes:** String literals: `''`; identifiers: `""`.

**Configuration:**
```ini
[sqruff:dialect:ansi]
```


### athena

Amazon Athena SQL dialect for querying data in S3.

**Default Casing:** `lowercase`

**Quotes:** String literals: `''`, `""`, or backticks; identifiers: `""` or backticks.

**Documentation:** [https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html](https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html)

**Configuration:**
```ini
[sqruff:dialect:athena]
```


### bigquery

Google BigQuery SQL dialect for analytics and data warehousing.

**Default Casing:** `UPPERCASE`

**Quotes:** String literals: `''`, `""`, `@`, or `@@`; quoted strings also support `r`/`R` raw or regex prefixes and `b`/`B` byte-string prefixes. Identifiers: `""` or backticks. Unquoted aliases resolve case-insensitively but retain their case in result sets.

**Documentation:** [https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax](https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax)

**Configuration:**
```ini
[sqruff:dialect:bigquery]
```


### clickhouse

ClickHouse SQL dialect for real-time analytics.

**Documentation:** [https://clickhouse.com/docs/en/sql-reference/](https://clickhouse.com/docs/en/sql-reference/)

**Configuration:**
```ini
[sqruff:dialect:clickhouse]
```


### databricks

Databricks SQL dialect for lakehouse analytics.

**Documentation:** [https://docs.databricks.com/en/sql/language-manual/index.html](https://docs.databricks.com/en/sql/language-manual/index.html)

**Configuration:**
```ini
[sqruff:dialect:databricks]
```


### db2

IBM Db2 SQL dialect.

**Documentation:** [https://www.ibm.com/docs/en/i/7.4?topic=overview-db2-i](https://www.ibm.com/docs/en/i/7.4?topic=overview-db2-i)

**Configuration:**
```ini
[sqruff:dialect:db2]
```


### duckdb

DuckDB SQL dialect for in-process analytical database.

**Default Casing:** DuckDB stores all identifiers in the case they were defined, but resolves both quoted and unquoted identifiers case-insensitively. See the [DuckDB identifiers documentation](https://duckdb.org/docs/sql/dialect/keywords_and_identifiers).

**Quotes:** String literals: `''`; identifiers: `""` or `''`.

**Documentation:** [https://duckdb.org/docs/sql/introduction](https://duckdb.org/docs/sql/introduction)

**Configuration:**
```ini
[sqruff:dialect:duckdb]
```


### exasol

Exasol SQL dialect for the Exasol analytics database.

**Documentation:** [https://docs.exasol.com/db/latest/sql_references.htm](https://docs.exasol.com/db/latest/sql_references.htm)

**Configuration:**
```ini
[sqruff:dialect:exasol]
```


### greenplum

Greenplum SQL dialect, a massively parallel Postgres.

**Documentation:** [https://docs.vmware.com/en/VMware-Greenplum/index.html](https://docs.vmware.com/en/VMware-Greenplum/index.html)

**Configuration:**
```ini
[sqruff:dialect:greenplum]
```


### hive

Apache Hive SQL dialect for data warehousing.

**Documentation:** [https://hive.apache.org/docs/latest/language/](https://hive.apache.org/docs/latest/language/)

**Configuration:**
```ini
[sqruff:dialect:hive]
```


### mariadb

MariaDB SQL dialect, a community-developed fork of MySQL.

**Default Casing:** `lowercase`

**Quotes:** String literals: `''`, `""`, or `@`; identifiers: backticks.

**Documentation:** [https://mariadb.com/kb/en/sql-statements-structure/](https://mariadb.com/kb/en/sql-statements-structure/)

**Configuration:**
```ini
[sqruff:dialect:mariadb]
```


### materialize

Materialize SQL dialect for the streaming data warehouse.

**Documentation:** [https://materialize.com/docs/sql/](https://materialize.com/docs/sql/)

**Configuration:**
```ini
[sqruff:dialect:materialize]
```


### mysql

MySQL SQL dialect for the popular open-source database.

**Default Casing:** `lowercase`

**Quotes:** String literals: `''`, `""`, or `@`; identifiers: backticks.

**Documentation:** [https://dev.mysql.com/doc/](https://dev.mysql.com/doc/)

**Configuration:**
```ini
[sqruff:dialect:mysql]
```


### oracle

Oracle SQL dialect for Oracle Database.

**Documentation:** [https://www.oracle.com/database/technologies/appdev/sql.html](https://www.oracle.com/database/technologies/appdev/sql.html)

**Configuration:**
```ini
[sqruff:dialect:oracle]
```


### postgres

PostgreSQL SQL dialect for the advanced open-source database.

**Default Casing:** `lowercase`

**Quotes:** String literals: `''`; identifiers: `""`.

**Documentation:** [https://www.postgresql.org/docs/current/sql.html](https://www.postgresql.org/docs/current/sql.html)

**Configuration:**
```ini
[sqruff:dialect:postgres]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `pg_trgm` | Enable parsing of pg_trgm trigram operators (%, <%, %>, <->, etc.) | `false` |
| `pgvector` | Enable parsing of pgvector data types (VECTOR, HALFVEC, SPARSEVEC). | `false` |

**Example:**
```ini
[sqruff:dialect:postgres]
pg_trgm = true
pgvector = true
```


### redshift

Amazon Redshift SQL dialect for cloud data warehousing.

**Default Casing:** `lowercase`, unless case-sensitive identifiers are enabled and all identifiers use the `enable_case_sensitive_identifier` configuration value. See the [Redshift names and identifiers documentation](https://spark.apache.org/docs/latest/sql-ref.html).

**Quotes:** String literals: `''`; identifiers: `""`.

**Documentation:** [https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html](https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html)

**Configuration:**
```ini
[sqruff:dialect:redshift]
```


### snowflake

Snowflake SQL dialect for cloud data platform.

**Default Casing:** `UPPERCASE`

**Quotes:** String literals: `''`; identifiers: `""`.

**Documentation:** [https://docs.snowflake.com/en/sql-reference.html](https://docs.snowflake.com/en/sql-reference.html)

**Configuration:**
```ini
[sqruff:dialect:snowflake]
```


### sparksql

Apache Spark SQL dialect for big data processing.

**Default Casing:** Spark SQL resolves both quoted and unquoted (*delimited*) identifiers case-insensitively. See the [Spark identifiers documentation](https://spark.apache.org/docs/latest/sql-ref-identifier.html).

**Quotes:** String literals: `''` or `""`; identifiers: backticks.

**Documentation:** [https://spark.apache.org/sql/](https://spark.apache.org/sql/)

**Configuration:**
```ini
[sqruff:dialect:sparksql]
```


### sqlite

SQLite SQL dialect for embedded database.

**Default Casing:** SQLite does not specify a default in its documentation. Testing indicates that it stores column names in their declared case but resolves them case-insensitively.

**Quotes:** String literals: `''` (or `""` when not resolved as an identifier); identifiers: `""`, `[]`, or backticks. See the [SQLite keywords documentation](https://sqlite.org/lang_keywords.html).

**Documentation:** [https://www.sqlite.org/lang.html](https://www.sqlite.org/lang.html)

**Configuration:**
```ini
[sqruff:dialect:sqlite]
```


### starrocks

StarRocks SQL dialect for real-time analytical workloads.

**Documentation:** [https://docs.starrocks.io/docs/sql-reference/](https://docs.starrocks.io/docs/sql-reference/)

**Configuration:**
```ini
[sqruff:dialect:starrocks]
```


### teradata

Teradata SQL dialect for the Teradata analytics platform.

**Documentation:** [https://docs.teradata.com/](https://docs.teradata.com/)

**Configuration:**
```ini
[sqruff:dialect:teradata]
```


### trino

Trino (formerly PrestoSQL) dialect for distributed SQL queries.

**Default Casing:** `UPPERCASE`

**Quotes:** String literals: `''`; identifiers: `""`.

**Documentation:** [https://trino.io/docs/current/sql.html](https://trino.io/docs/current/sql.html)

**Configuration:**
```ini
[sqruff:dialect:trino]
```


### tsql

T-SQL dialect for Microsoft SQL Server and Azure SQL.

**Documentation:** [https://learn.microsoft.com/en-us/sql/t-sql/language-reference](https://learn.microsoft.com/en-us/sql/t-sql/language-reference)

**Configuration:**
```ini
[sqruff:dialect:tsql]
```


### vertica

Vertica SQL dialect for the columnar analytics database.

**Documentation:** [https://docs.vertica.com/latest/en/](https://docs.vertica.com/latest/en/)

**Configuration:**
```ini
[sqruff:dialect:vertica]
```


We are working on adding support for more dialects in the future.