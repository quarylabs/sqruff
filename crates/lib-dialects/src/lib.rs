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
#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "hive")]
pub mod hive;
#[cfg(feature = "mysql")]
pub mod mysql;
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
        #[cfg(feature = "duckdb")]
        DialectKind::Duckdb => duckdb::dialect(config),
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => mysql::dialect(config),
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
