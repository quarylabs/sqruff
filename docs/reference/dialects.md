# Dialects

Sqruff currently supports the following SQL dialects:

## Dialects Index

- [ansi](#ansi)
- [athena](#athena)
- [bigquery](#bigquery)
- [clickhouse](#clickhouse)
- [databricks](#databricks)
- [duckdb](#duckdb)
- [mysql](#mysql)
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


### athena

Amazon Athena SQL dialect for querying data in S3.

**Documentation:** [https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html](https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html)


### bigquery

Google BigQuery SQL dialect for analytics and data warehousing.

**Documentation:** [https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax](https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax)


### clickhouse

ClickHouse SQL dialect for real-time analytics.

**Documentation:** [https://clickhouse.com/docs/en/sql-reference/](https://clickhouse.com/docs/en/sql-reference/)


### databricks

Databricks SQL dialect for lakehouse analytics.

**Documentation:** [https://docs.databricks.com/en/sql/language-manual/index.html](https://docs.databricks.com/en/sql/language-manual/index.html)


### duckdb

DuckDB SQL dialect for in-process analytical database.

**Documentation:** [https://duckdb.org/docs/sql/introduction](https://duckdb.org/docs/sql/introduction)


### mysql

MySQL SQL dialect for the popular open-source database.

**Documentation:** [https://dev.mysql.com/doc/](https://dev.mysql.com/doc/)


### postgres

PostgreSQL SQL dialect for the advanced open-source database.

**Documentation:** [https://www.postgresql.org/docs/current/sql.html](https://www.postgresql.org/docs/current/sql.html)


### redshift

Amazon Redshift SQL dialect for cloud data warehousing.

**Documentation:** [https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html](https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html)


### snowflake

Snowflake SQL dialect for cloud data platform.

**Documentation:** [https://docs.snowflake.com/en/sql-reference.html](https://docs.snowflake.com/en/sql-reference.html)


### sparksql

Apache Spark SQL dialect for big data processing.

**Documentation:** [https://spark.apache.org/sql/](https://spark.apache.org/sql/)


### sqlite

SQLite SQL dialect for embedded database.

**Documentation:** [https://www.sqlite.org/lang.html](https://www.sqlite.org/lang.html)


### trino

Trino (formerly PrestoSQL) dialect for distributed SQL queries.

**Documentation:** [https://trino.io/docs/current/sql.html](https://trino.io/docs/current/sql.html)


### tsql

T-SQL dialect for Microsoft SQL Server and Azure SQL.

**Documentation:** [https://learn.microsoft.com/en-us/sql/t-sql/language-reference](https://learn.microsoft.com/en-us/sql/t-sql/language-reference)


We are working on adding support for more dialects in the future.