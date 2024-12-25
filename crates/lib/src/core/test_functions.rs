use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, Tables};
use sqruff_lib_dialects::kind_to_dialect;

use crate::core::linter::core::Linter;

pub fn parse_ansi_string(sql: &str) -> ErasedSegment {
    let tables = Tables::default();
    let linter = Linter::new(<_>::default(), None, None, false);
    linter
        .parse_string(&tables, sql, None)
        .unwrap()
        .tree
        .unwrap()
}

pub fn fresh_ansi_dialect() -> Dialect {
    kind_to_dialect(&DialectKind::Ansi).unwrap()
}
