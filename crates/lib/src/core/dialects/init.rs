use strum::IntoEnumIterator;
use strum_macros::AsRefStr;

use super::base::Dialect;

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
    Hash
)]
#[strum(serialize_all = "snake_case")]
pub enum DialectKind {
    #[default]
    Ansi,
    Bigquery,
    Postgres,
    Snowflake,
    Clickhouse,
    Sparksql,
    Duckdb,
    Sqlite,
    Redshift,
}

impl From<DialectKind> for Dialect {
    fn from(val: DialectKind) -> Self {
        match val {
            DialectKind::Ansi => crate::dialects::ansi::dialect(),
            DialectKind::Bigquery => crate::dialects::bigquery::dialect(),
            DialectKind::Postgres => crate::dialects::postgres::dialect(),
            DialectKind::Snowflake => crate::dialects::snowflake::dialect(),
            DialectKind::Clickhouse => crate::dialects::clickhouse::dialect(),
            DialectKind::Sparksql => crate::dialects::sparksql::dialect(),
            DialectKind::Duckdb => crate::dialects::duckdb::dialect(),
            DialectKind::Sqlite => crate::dialects::sqlite::dialect(),
            DialectKind::Redshift => crate::dialects::redshift::dialect(),
        }
    }
}

/// Generate a readout of available dialects.
pub fn dialect_readout() -> Vec<String> {
    DialectKind::iter().map(|x| x.as_ref().to_string()).collect()
}
