use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;

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

pub fn kind_to_dialect(kind: &DialectKind) -> Option<Dialect> {
    #[allow(unreachable_patterns)]
    Some(match kind {
        DialectKind::Ansi => ansi::dialect(),
        #[cfg(feature = "athena")]
        DialectKind::Athena => athena::dialect(),
        #[cfg(feature = "bigquery")]
        DialectKind::Bigquery => bigquery::dialect(),
        #[cfg(feature = "clickhouse")]
        DialectKind::Clickhouse => clickhouse::dialect(),
        #[cfg(feature = "databricks")]
        DialectKind::Databricks => databricks::dialect(),
        #[cfg(feature = "duckdb")]
        DialectKind::Duckdb => duckdb::dialect(),
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => mysql::dialect(),
        #[cfg(feature = "postgres")]
        DialectKind::Postgres => postgres::dialect(),
        #[cfg(feature = "redshift")]
        DialectKind::Redshift => redshift::dialect(),
        #[cfg(feature = "snowflake")]
        DialectKind::Snowflake => snowflake::dialect(),
        #[cfg(feature = "sparksql")]
        DialectKind::Sparksql => sparksql::dialect(),
        #[cfg(feature = "sqlite")]
        DialectKind::Sqlite => sqlite::dialect(),
        #[cfg(feature = "trino")]
        DialectKind::Trino => trino::dialect(),
        _ => return None,
    })
}
