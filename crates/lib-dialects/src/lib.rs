use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;

pub mod ansi;
mod ansi_keywords;
pub mod athena;
mod athena_keywords;
pub mod bigquery;
mod bigquery_keywords;
pub mod clickhouse;
mod clickhouse_keywords;
pub mod duckdb;
pub mod hive;
pub mod postgres;
mod postgres_keywords;
pub mod redshift;
mod redshift_keywords;
pub mod snowflake;
mod snowflake_keywords;
pub mod sparksql;
mod sparksql_keywords;
pub mod sqlite;
mod sqlite_keywords;
pub mod trino;
mod trino_keywords;

pub fn kind_to_dialect(kind: &DialectKind) -> Dialect {
    match kind {
        DialectKind::Ansi => ansi::dialect(),
        DialectKind::Athena => athena::dialect(),
        DialectKind::Bigquery => bigquery::dialect(),
        DialectKind::Clickhouse => clickhouse::dialect(),
        DialectKind::Duckdb => duckdb::dialect(),
        DialectKind::Postgres => postgres::dialect(),
        DialectKind::Redshift => redshift::dialect(),
        DialectKind::Snowflake => snowflake::dialect(),
        DialectKind::Sparksql => sparksql::dialect(),
        DialectKind::Sqlite => sqlite::dialect(),
        DialectKind::Trino => trino::dialect(),
    }
}
