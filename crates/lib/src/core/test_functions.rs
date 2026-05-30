use std::borrow::Cow;

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_dialects::{DialectConfigs, kind_to_dialect};

use crate::api::{Engine, EngineOptions, ParseErrors, Source, SourceId};

pub fn parse_ansi_string(sql: &str) -> ErasedSegment {
    Engine::new(
        <_>::default(),
        EngineOptions {
            parse_errors: ParseErrors::Suppress,
        },
    )
    .unwrap()
    .parse_source(Source {
        id: SourceId::Virtual("test.sql".into()),
        text: Cow::Borrowed(sql),
    })
    .unwrap()
    .tree
    .unwrap()
}

pub fn fresh_ansi_dialect() -> Dialect {
    kind_to_dialect(&DialectKind::Ansi, &DialectConfigs::default()).unwrap()
}
