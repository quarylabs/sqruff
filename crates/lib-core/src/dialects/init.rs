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
