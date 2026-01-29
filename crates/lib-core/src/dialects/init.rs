use strum::IntoEnumIterator;
use strum_macros::AsRefStr;

#[derive(
    strum_macros::EnumString,
    strum_macros::EnumIter,
    AsRefStr,
    Debug,
    Clone,
    Copy,
    Default,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
)]
#[strum(serialize_all = "snake_case")]
pub enum DialectKind {
    #[default]
    Ansi,
    Athena,
    Bigquery,
    Clickhouse,
    Databricks,
    Duckdb,
    Mysql,
    Postgres,
    Redshift,
    Snowflake,
    Sparksql,
    Sqlite,
    Trino,
    Tsql,
}

impl DialectKind {
    /// Returns the human-readable name of the dialect.
    pub fn name(&self) -> &'static str {
        match self {
            DialectKind::Ansi => "ansi",
            DialectKind::Athena => "athena",
            DialectKind::Bigquery => "bigquery",
            DialectKind::Clickhouse => "clickhouse",
            DialectKind::Databricks => "databricks",
            DialectKind::Duckdb => "duckdb",
            DialectKind::Mysql => "mysql",
            DialectKind::Postgres => "postgres",
            DialectKind::Redshift => "redshift",
            DialectKind::Snowflake => "snowflake",
            DialectKind::Sparksql => "sparksql",
            DialectKind::Sqlite => "sqlite",
            DialectKind::Trino => "trino",
            DialectKind::Tsql => "tsql",
        }
    }

    /// Returns a human-readable description of the dialect.
    pub fn description(&self) -> &'static str {
        match self {
            DialectKind::Ansi => {
                "Standard SQL syntax. The default dialect and base for all others."
            }
            DialectKind::Athena => "Amazon Athena SQL dialect for querying data in S3.",
            DialectKind::Bigquery => {
                "Google BigQuery SQL dialect for analytics and data warehousing."
            }
            DialectKind::Clickhouse => "ClickHouse SQL dialect for real-time analytics.",
            DialectKind::Databricks => "Databricks SQL dialect for lakehouse analytics.",
            DialectKind::Duckdb => "DuckDB SQL dialect for in-process analytical database.",
            DialectKind::Mysql => "MySQL SQL dialect for the popular open-source database.",
            DialectKind::Postgres => {
                "PostgreSQL SQL dialect for the advanced open-source database."
            }
            DialectKind::Redshift => "Amazon Redshift SQL dialect for cloud data warehousing.",
            DialectKind::Snowflake => "Snowflake SQL dialect for cloud data platform.",
            DialectKind::Sparksql => "Apache Spark SQL dialect for big data processing.",
            DialectKind::Sqlite => "SQLite SQL dialect for embedded database.",
            DialectKind::Trino => "Trino (formerly PrestoSQL) dialect for distributed SQL queries.",
            DialectKind::Tsql => "T-SQL dialect for Microsoft SQL Server and Azure SQL.",
        }
    }

    /// Returns an optional URL to the official documentation for the dialect.
    pub fn doc_url(&self) -> Option<&'static str> {
        match self {
            DialectKind::Ansi => None,
            DialectKind::Athena => {
                Some("https://docs.aws.amazon.com/athena/latest/ug/ddl-sql-reference.html")
            }
            DialectKind::Bigquery => {
                Some("https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax")
            }
            DialectKind::Clickhouse => Some("https://clickhouse.com/docs/en/sql-reference/"),
            DialectKind::Databricks => {
                Some("https://docs.databricks.com/en/sql/language-manual/index.html")
            }
            DialectKind::Duckdb => Some("https://duckdb.org/docs/sql/introduction"),
            DialectKind::Mysql => Some("https://dev.mysql.com/doc/"),
            DialectKind::Postgres => Some("https://www.postgresql.org/docs/current/sql.html"),
            DialectKind::Redshift => {
                Some("https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html")
            }
            DialectKind::Snowflake => Some("https://docs.snowflake.com/en/sql-reference.html"),
            DialectKind::Sparksql => Some("https://spark.apache.org/sql/"),
            DialectKind::Sqlite => Some("https://www.sqlite.org/lang.html"),
            DialectKind::Trino => Some("https://trino.io/docs/current/sql.html"),
            DialectKind::Tsql => {
                Some("https://learn.microsoft.com/en-us/sql/t-sql/language-reference")
            }
        }
    }
}

/// Generate a readout of available dialects.
pub fn dialect_readout() -> Vec<String> {
    DialectKind::iter()
        .map(|x| x.as_ref().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn dialect_readout_is_alphabetically_sorted() {
        let readout = super::dialect_readout();

        let mut sorted = readout.clone();
        sorted.sort();

        assert_eq!(readout, sorted);
    }
}
