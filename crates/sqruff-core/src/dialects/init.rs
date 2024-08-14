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
    Sqlite,
    Redshift,
}

pub fn dialect_selector(_s: &str) -> Option<Dialect> {
    todo!()
}

pub fn get_default_dialect() -> &'static str {
    "ansi"
}
