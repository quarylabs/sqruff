use crate::core::dialects::base::Dialect;

pub fn raw_dialect() -> Dialect {
    let hive_dialect = super::ansi::ansi_dialect();

    hive_dialect
}
