use strum::IntoEnumIterator;
use strum_macros::AsRefStr;

use crate::value::Value;

/// Trait for dialect-specific configuration.
/// Each dialect implements this to parse and validate its configuration from raw config values.
pub trait DialectConfig: Default + Clone + std::fmt::Debug {
    /// Parse configuration from a Value (typically a Map from the config file's dialect section).
    /// Returns the default configuration if parsing fails or if the input is None.
    fn from_value(value: &Value) -> Self {
        let _ = value;
        Self::default()
    }
}

/// Macro to generate a dialect config struct with `DialectConfig` impl and `config_options()`.
///
/// # Usage
///
/// ```ignore
/// // Dialect with config options (all bool fields):
/// sqruff_lib_core::dialect_config!(PostgresDialectConfig {
///     /// Enable pg_trgm operators
///     pg_trgm: "Enable parsing of pg_trgm trigram operators"
/// });
///
/// // Dialect with no config options:
/// sqruff_lib_core::dialect_config!(AnsiDialectConfig {});
/// ```
#[macro_export]
macro_rules! dialect_config {
    // With fields (all bool)
    ($name:ident { $(
        $(#[doc = $doc:expr])*
        $field:ident : $desc:expr
    ),* $(,)? }) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            $($(#[doc = $doc])* pub $field: bool,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: false,)* }
            }
        }

        impl $crate::dialects::init::DialectConfig for $name {
            fn from_value(value: &$crate::value::Value) -> Self {
                Self {
                    $($field: value[stringify!($field)].to_bool(),)*
                }
            }
        }

        impl $name {
            pub fn config_options() -> Vec<(&'static str, &'static str, &'static str)> {
                vec![
                    $((stringify!($field), $desc, "false"),)*
                ]
            }
        }
    };
    // No fields
    ($name:ident {}) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name;

        impl $crate::dialects::init::DialectConfig for $name {}

        impl $name {
            pub fn config_options() -> Vec<(&'static str, &'static str, &'static str)> {
                vec![]
            }
        }
    };
}

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
    Db2,
    Duckdb,
    Exasol,
    Greenplum,
    Hive,
    Mariadb,
    Materialize,
    Mysql,
    Oracle,
    Postgres,
    Redshift,
    Snowflake,
    Sparksql,
    Sqlite,
    Starrocks,
    Teradata,
    Trino,
    Tsql,
    Vertica,
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
            DialectKind::Db2 => "db2",
            DialectKind::Duckdb => "duckdb",
            DialectKind::Exasol => "exasol",
            DialectKind::Greenplum => "greenplum",
            DialectKind::Hive => "hive",
            DialectKind::Mariadb => "mariadb",
            DialectKind::Mysql => "mysql",
            DialectKind::Materialize => "materialize",
            DialectKind::Oracle => "oracle",
            DialectKind::Postgres => "postgres",
            DialectKind::Redshift => "redshift",
            DialectKind::Snowflake => "snowflake",
            DialectKind::Sparksql => "sparksql",
            DialectKind::Sqlite => "sqlite",
            DialectKind::Starrocks => "starrocks",
            DialectKind::Teradata => "teradata",
            DialectKind::Trino => "trino",
            DialectKind::Tsql => "tsql",
            DialectKind::Vertica => "vertica",
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
            DialectKind::Db2 => "IBM Db2 SQL dialect.",
            DialectKind::Duckdb => "DuckDB SQL dialect for in-process analytical database.",
            DialectKind::Exasol => "Exasol SQL dialect for the Exasol analytics database.",
            DialectKind::Greenplum => "Greenplum SQL dialect, a massively parallel Postgres.",
            DialectKind::Hive => "Apache Hive SQL dialect for data warehousing.",
            DialectKind::Mariadb => "MariaDB SQL dialect, a community-developed fork of MySQL.",
            DialectKind::Mysql => "MySQL SQL dialect for the popular open-source database.",
            DialectKind::Materialize => "Materialize SQL dialect for the streaming data warehouse.",
            DialectKind::Oracle => "Oracle SQL dialect for Oracle Database.",
            DialectKind::Postgres => {
                "PostgreSQL SQL dialect for the advanced open-source database."
            }
            DialectKind::Redshift => "Amazon Redshift SQL dialect for cloud data warehousing.",
            DialectKind::Snowflake => "Snowflake SQL dialect for cloud data platform.",
            DialectKind::Sparksql => "Apache Spark SQL dialect for big data processing.",
            DialectKind::Sqlite => "SQLite SQL dialect for embedded database.",
            DialectKind::Starrocks => "StarRocks SQL dialect for real-time analytical workloads.",
            DialectKind::Teradata => "Teradata SQL dialect for the Teradata analytics platform.",
            DialectKind::Trino => "Trino (formerly PrestoSQL) dialect for distributed SQL queries.",
            DialectKind::Tsql => "T-SQL dialect for Microsoft SQL Server and Azure SQL.",
            DialectKind::Vertica => "Vertica SQL dialect for the columnar analytics database.",
        }
    }

    /// Returns documentation about how the dialect stores and resolves identifier casing.
    pub fn default_casing(&self) -> Option<&'static str> {
        match self {
            DialectKind::Ansi
            | DialectKind::Bigquery
            | DialectKind::Snowflake
            | DialectKind::Trino => Some("`UPPERCASE`"),
            DialectKind::Athena
            | DialectKind::Mariadb
            | DialectKind::Mysql
            | DialectKind::Postgres => Some("`lowercase`"),
            DialectKind::Duckdb => Some(concat!(
                "DuckDB stores all identifiers in the case they were defined, but resolves ",
                "both quoted and unquoted identifiers case-insensitively. See the ",
                "[DuckDB identifiers documentation](https://duckdb.org/docs/sql/dialect/keywords_and_identifiers)."
            )),
            DialectKind::Redshift => Some(concat!(
                "`lowercase`, unless case-sensitive identifiers are enabled and all identifiers ",
                "use the `enable_case_sensitive_identifier` configuration value. See the ",
                "[Redshift names and identifiers documentation](https://spark.apache.org/docs/latest/sql-ref.html)."
            )),
            DialectKind::Sparksql => Some(concat!(
                "Spark SQL resolves both quoted and unquoted (*delimited*) identifiers ",
                "case-insensitively. See the ",
                "[Spark identifiers documentation](https://spark.apache.org/docs/latest/sql-ref-identifier.html)."
            )),
            DialectKind::Sqlite => Some(concat!(
                "SQLite does not specify a default in its documentation. Testing indicates that ",
                "it stores column names in their declared case but resolves them case-insensitively."
            )),
            DialectKind::Clickhouse
            | DialectKind::Databricks
            | DialectKind::Db2
            | DialectKind::Exasol
            | DialectKind::Greenplum
            | DialectKind::Hive
            | DialectKind::Materialize
            | DialectKind::Oracle
            | DialectKind::Starrocks
            | DialectKind::Teradata
            | DialectKind::Tsql
            | DialectKind::Vertica => None,
        }
    }

    /// Returns documentation about string-literal and identifier quote styles.
    pub fn quotes(&self) -> Option<&'static str> {
        match self {
            DialectKind::Ansi
            | DialectKind::Postgres
            | DialectKind::Redshift
            | DialectKind::Snowflake
            | DialectKind::Trino => Some("String literals: `''`; identifiers: `\"\"`."),
            DialectKind::Athena => Some(
                "String literals: `''`, `\"\"`, or backticks; identifiers: `\"\"` or backticks.",
            ),
            DialectKind::Bigquery => Some(concat!(
                "String literals: `''`, `\"\"`, `@`, or `@@`; quoted strings also support ",
                "`r`/`R` raw or regex prefixes and `b`/`B` byte-string prefixes. Identifiers: ",
                "`\"\"` or backticks. Unquoted aliases resolve case-insensitively but retain ",
                "their case in result sets."
            )),
            DialectKind::Duckdb => Some("String literals: `''`; identifiers: `\"\"` or `''`."),
            DialectKind::Mariadb | DialectKind::Mysql => {
                Some("String literals: `''`, `\"\"`, or `@`; identifiers: backticks.")
            }
            DialectKind::Sparksql => {
                Some("String literals: `''` or `\"\"`; identifiers: backticks.")
            }
            DialectKind::Sqlite => Some(concat!(
                "String literals: `''` (or `\"\"` when not resolved as an identifier); ",
                "identifiers: `\"\"`, `[]`, or backticks. See the ",
                "[SQLite keywords documentation](https://sqlite.org/lang_keywords.html)."
            )),
            DialectKind::Clickhouse
            | DialectKind::Databricks
            | DialectKind::Db2
            | DialectKind::Exasol
            | DialectKind::Greenplum
            | DialectKind::Hive
            | DialectKind::Materialize
            | DialectKind::Oracle
            | DialectKind::Starrocks
            | DialectKind::Teradata
            | DialectKind::Tsql
            | DialectKind::Vertica => None,
        }
    }

    /// Returns the configuration section header for this dialect.
    /// Format: `[sqruff:dialect:{dialect_name}]`
    pub fn config_section(&self) -> String {
        format!("[sqruff:dialect:{}]", self.name())
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
            DialectKind::Db2 => Some("https://www.ibm.com/docs/en/i/7.4?topic=overview-db2-i"),
            DialectKind::Duckdb => Some("https://duckdb.org/docs/sql/introduction"),
            DialectKind::Exasol => Some("https://docs.exasol.com/db/latest/sql_references.htm"),
            DialectKind::Greenplum => {
                Some("https://docs.vmware.com/en/VMware-Greenplum/index.html")
            }
            DialectKind::Hive => Some("https://hive.apache.org/docs/latest/language/"),
            DialectKind::Mariadb => Some("https://mariadb.com/kb/en/sql-statements-structure/"),
            DialectKind::Mysql => Some("https://dev.mysql.com/doc/"),
            DialectKind::Materialize => Some("https://materialize.com/docs/sql/"),
            DialectKind::Oracle => {
                Some("https://www.oracle.com/database/technologies/appdev/sql.html")
            }
            DialectKind::Postgres => Some("https://www.postgresql.org/docs/current/sql.html"),
            DialectKind::Redshift => {
                Some("https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html")
            }
            DialectKind::Snowflake => Some("https://docs.snowflake.com/en/sql-reference.html"),
            DialectKind::Sparksql => Some("https://spark.apache.org/sql/"),
            DialectKind::Sqlite => Some("https://www.sqlite.org/lang.html"),
            DialectKind::Starrocks => Some("https://docs.starrocks.io/docs/sql-reference/"),
            DialectKind::Teradata => Some("https://docs.teradata.com/"),
            DialectKind::Trino => Some("https://trino.io/docs/current/sql.html"),
            DialectKind::Tsql => {
                Some("https://learn.microsoft.com/en-us/sql/t-sql/language-reference")
            }
            DialectKind::Vertica => Some("https://docs.vertica.com/latest/en/"),
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
    use super::DialectKind;

    #[test]
    fn dialect_readout_is_alphabetically_sorted() {
        let readout = super::dialect_readout();

        let mut sorted = readout.clone();
        sorted.sort();

        assert_eq!(readout, sorted);
    }

    #[test]
    fn config_section_format() {
        assert_eq!(
            DialectKind::Snowflake.config_section(),
            "[sqruff:dialect:snowflake]"
        );
        assert_eq!(
            DialectKind::Bigquery.config_section(),
            "[sqruff:dialect:bigquery]"
        );
        assert_eq!(DialectKind::Ansi.config_section(), "[sqruff:dialect:ansi]");
    }

    #[test]
    fn dialect_documentation_metadata() {
        assert_eq!(DialectKind::Postgres.default_casing(), Some("`lowercase`"));
        assert_eq!(
            DialectKind::Postgres.quotes(),
            Some("String literals: `''`; identifiers: `\"\"`.")
        );
        assert_eq!(DialectKind::Tsql.default_casing(), None);
        assert_eq!(DialectKind::Tsql.quotes(), None);
    }
}
