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
    Athena,
    Bigquery,
    Clickhouse,
    Duckdb,
    Postgres,
    Redshift,
    Snowflake,
    Sparksql,
    Sqlite,
    Trino,
}

impl From<DialectKind> for Dialect {
    fn from(val: DialectKind) -> Self {
        match val {
            DialectKind::Ansi => crate::dialects::ansi::dialect(),
            DialectKind::Athena => crate::dialects::athena::dialect(),
            DialectKind::Bigquery => crate::dialects::bigquery::dialect(),
            DialectKind::Clickhouse => crate::dialects::clickhouse::dialect(),
            DialectKind::Duckdb => crate::dialects::duckdb::dialect(),
            DialectKind::Postgres => crate::dialects::postgres::dialect(),
            DialectKind::Redshift => crate::dialects::redshift::dialect(),
            DialectKind::Snowflake => crate::dialects::snowflake::dialect(),
            DialectKind::Sparksql => crate::dialects::sparksql::dialect(),
            DialectKind::Sqlite => crate::dialects::sqlite::dialect(),
            DialectKind::Trino => crate::dialects::trino::dialect(),
        }
    }
}

/// Generate a readout of available dialects.
pub fn dialect_readout() -> Vec<String> {
    DialectKind::iter().map(|x| x.as_ref().to_string()).collect()
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
