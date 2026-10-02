//! Shared TPC-H and TPC-DS fixture loading for benchmarks and timing examples.

use std::fs;
use std::path::PathBuf;

pub(crate) const TPCH_QUERIES: u32 = 22;
pub(crate) const TPCDS_QUERIES: u32 = 99;

fn fixture_root() -> PathBuf {
    std::env::var_os("SQRUFF_TPC_FIXTURES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tpc-fixtures")
        })
}

pub(crate) fn load_suite(suite: &str, count: u32) -> Vec<(u32, String)> {
    let root = fixture_root();
    (1..=count)
        .map(|n| {
            let path = root.join(suite).join(format!("{n}.sql"));
            let sql = fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "Cannot read TPC query {}: {error}. Run .hacking/scripts/fetch_tpc_queries.sh first, or set SQRUFF_TPC_FIXTURES_DIR.",
                    path.display()
                )
            });
            (n, sql)
        })
        .collect()
}
