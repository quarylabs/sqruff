// Minimal T-SQL dialect for testing
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use crate::ansi;

pub fn minimal_tsql_dialect() -> Dialect {
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Tsql;
    dialect
}