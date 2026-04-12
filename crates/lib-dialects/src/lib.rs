use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::value::Value;

pub mod ansi;
mod ansi_keywords;
#[cfg(feature = "athena")]
pub mod athena;
#[cfg(feature = "athena")]
mod athena_keywords;
#[cfg(feature = "bigquery")]
pub mod bigquery;
#[cfg(feature = "bigquery")]
mod bigquery_keywords;
#[cfg(feature = "clickhouse")]
pub mod clickhouse;
#[cfg(feature = "clickhouse")]
mod clickhouse_keywords;
#[cfg(feature = "databricks")]
pub mod databricks;
#[cfg(feature = "databricks")]
pub mod databricks_keywords;
#[cfg(feature = "db2")]
pub mod db2;
#[cfg(feature = "db2")]
mod db2_keywords;
#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "hive")]
pub mod hive;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "mysql")]
mod mysql_keywords;
#[cfg(feature = "oracle")]
pub mod oracle;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "postgres")]
mod postgres_keywords;
#[cfg(feature = "redshift")]
pub mod redshift;
#[cfg(feature = "redshift")]
mod redshift_keywords;
#[cfg(feature = "snowflake")]
pub mod snowflake;
#[cfg(feature = "snowflake")]
mod snowflake_keywords;
#[cfg(feature = "sparksql")]
pub mod sparksql;
#[cfg(feature = "sparksql")]
mod sparksql_keywords;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "sqlite")]
mod sqlite_keywords;
#[cfg(feature = "trino")]
pub mod trino;
#[cfg(feature = "trino")]
mod trino_keywords;
#[cfg(feature = "tsql")]
pub mod tsql;
#[cfg(feature = "tsql")]
mod tsql_keywords;

/// Returns dialect-specific configuration options for the given dialect kind.
/// Each entry is (option_name, description, default_value).
pub fn dialect_config_options(
    kind: &DialectKind,
) -> Vec<(&'static str, &'static str, &'static str)> {
    #[allow(unreachable_patterns)]
    match kind {
        DialectKind::Ansi => ansi::AnsiDialectConfig::config_options(),
        #[cfg(feature = "athena")]
        DialectKind::Athena => athena::AthenaDialectConfig::config_options(),
        #[cfg(feature = "bigquery")]
        DialectKind::Bigquery => bigquery::BigQueryDialectConfig::config_options(),
        #[cfg(feature = "clickhouse")]
        DialectKind::Clickhouse => clickhouse::ClickHouseDialectConfig::config_options(),
        #[cfg(feature = "databricks")]
        DialectKind::Databricks => databricks::DatabricksDialectConfig::config_options(),
        #[cfg(feature = "db2")]
        DialectKind::Db2 => db2::Db2DialectConfig::config_options(),
        #[cfg(feature = "duckdb")]
        DialectKind::Duckdb => duckdb::DuckDBDialectConfig::config_options(),
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => mysql::MySQLDialectConfig::config_options(),
        #[cfg(feature = "oracle")]
        DialectKind::Oracle => oracle::OracleDialectConfig::config_options(),
        #[cfg(feature = "postgres")]
        DialectKind::Postgres => postgres::PostgresDialectConfig::config_options(),
        #[cfg(feature = "redshift")]
        DialectKind::Redshift => redshift::RedshiftDialectConfig::config_options(),
        #[cfg(feature = "snowflake")]
        DialectKind::Snowflake => snowflake::SnowflakeDialectConfig::config_options(),
        #[cfg(feature = "sparksql")]
        DialectKind::Sparksql => sparksql::SparkSQLDialectConfig::config_options(),
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => sqlite::SQLiteDialectConfig::config_options(),
        #[cfg(feature = "trino")]
        DialectKind::Trino => trino::TrinoDialectConfig::config_options(),
        #[cfg(feature = "tsql")]
        DialectKind::Tsql => tsql::TSQLDialectConfig::config_options(),
        _ => vec![],
    }
}

pub fn kind_to_dialect(kind: &DialectKind, config: Option<&Value>) -> Option<Dialect> {
    #[allow(unreachable_patterns)]
    Some(match kind {
        DialectKind::Ansi => ansi::dialect(config),
        #[cfg(feature = "athena")]
        DialectKind::Athena => athena::dialect(config),
        #[cfg(feature = "bigquery")]
        DialectKind::Bigquery => bigquery::dialect(config),
        #[cfg(feature = "clickhouse")]
        DialectKind::Clickhouse => clickhouse::dialect(config),
        #[cfg(feature = "databricks")]
        DialectKind::Databricks => databricks::dialect(config),
        #[cfg(feature = "db2")]
        DialectKind::Db2 => db2::dialect(config),
        #[cfg(feature = "duckdb")]
        DialectKind::Duckdb => duckdb::dialect(config),
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => mysql::dialect(config),
        #[cfg(feature = "oracle")]
        DialectKind::Oracle => oracle::dialect(config),
        #[cfg(feature = "postgres")]
        DialectKind::Postgres => postgres::dialect(config),
        #[cfg(feature = "redshift")]
        DialectKind::Redshift => redshift::dialect(config),
        #[cfg(feature = "snowflake")]
        DialectKind::Snowflake => snowflake::dialect(config),
        #[cfg(feature = "sparksql")]
        DialectKind::Sparksql => sparksql::dialect(config),
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => sqlite::dialect(config),
        #[cfg(feature = "trino")]
        DialectKind::Trino => trino::dialect(config),
        #[cfg(feature = "tsql")]
        DialectKind::Tsql => tsql::dialect(config),
        _ => return None,
    })
}
