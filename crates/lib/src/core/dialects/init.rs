use std::str::FromStr;

use strum_macros::AsRefStr;

use super::base::Dialect;

#[derive(
    strum_macros::EnumString,
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
}

pub fn dialect_selector(s: &str) -> Option<Dialect> {
    match DialectKind::from_str(s).ok()? {
        DialectKind::Ansi => Some(crate::dialects::ansi::ansi_dialect()),
        DialectKind::Bigquery => Some(crate::dialects::bigquery::bigquery_dialect()),
        DialectKind::Postgres => Some(crate::dialects::postgres::dialect()),
        DialectKind::Snowflake => Some(crate::dialects::snowflake::snowflake_dialect()),
        DialectKind::Clickhouse => Some(crate::dialects::clickhouse::clickhouse_dialect()),
        DialectKind::Sparksql => Some(crate::dialects::sparksql::sparksql_dialect()),
        DialectKind::Duckdb => Some(crate::dialects::duckdb::dialect()),
    }
}

pub fn get_default_dialect() -> &'static str {
    "ansi"
}

/// Dialect Tuple object for describing dialects.
pub struct DialectTuple {
    pub label: String,
    pub name: String,
    pub inherits_from: String,
}

/// Generate a readout of available dialects.
pub fn dialect_readout() -> Vec<String> {
    panic!("dialect_readout not implemented yet");
}
