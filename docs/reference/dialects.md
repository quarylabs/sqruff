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
- [mysql](#mysql)
- [oracle](#oracle)
- [postgres](#postgres)
- [redshift](#redshift)
- [snowflake](#snowflake)
- [sparksql](#sparksql)
- [sqlite](#sqlite)
- [trino](#trino)
- [tsql](#tsql)

## Details

### ansi

Standard SQL syntax. The default dialect and base for all others.

**Configuration:**
```ini
[sqruff:dialect:ansi]
```


### athena

Amazon Athena SQL dialect for querying data in S3.

**Documentation:** [https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html](https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html)

**Configuration:**
```ini
[sqruff:dialect:athena]
```


### bigquery

Google BigQuery SQL dialect for analytics and data warehousing.

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

**Documentation:** [https://duckdb.org/docs/sql/introduction](https://duckdb.org/docs/sql/introduction)

**Configuration:**
```ini
[sqruff:dialect:duckdb]
```


### mysql

MySQL SQL dialect for the popular open-source database.

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

**Documentation:** [https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html](https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html)

**Configuration:**
```ini
[sqruff:dialect:redshift]
```


### snowflake

Snowflake SQL dialect for cloud data platform.

**Documentation:** [https://docs.snowflake.com/en/sql-reference.html](https://docs.snowflake.com/en/sql-reference.html)

**Configuration:**
```ini
[sqruff:dialect:snowflake]
```


### sparksql

Apache Spark SQL dialect for big data processing.

**Documentation:** [https://spark.apache.org/sql/](https://spark.apache.org/sql/)

**Configuration:**
```ini
[sqruff:dialect:sparksql]
```


### sqlite

SQLite SQL dialect for embedded database.

**Documentation:** [https://www.sqlite.org/lang.html](https://www.sqlite.org/lang.html)

**Configuration:**
```ini
[sqruff:dialect:sqlite]
```


### trino

Trino (formerly PrestoSQL) dialect for distributed SQL queries.

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


We are working on adding support for more dialects in the future.