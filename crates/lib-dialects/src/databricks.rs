use sqruff_lib_core::dialects::{base::Dialect, init::DialectKind};

pub fn dialect() -> Dialect {
    let sparksql = crate::sparksql::dialect();

    let mut databricks = sparksql;
    databricks.name = DialectKind::Databricks;

    return databricks;
}
