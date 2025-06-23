use sqruff_lib_core::parser::Parser;
use sqruff_lib_core::parser::lexer::StringOrTemplate;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};

pub mod aggregate_functions;
pub mod columns;
pub mod infer_tests;
pub mod inference;
pub mod test;

pub fn parse_sql(parser: &Parser, source: &str) -> ErasedSegment {
    let tables = Tables::default();
    let lexer = parser.dialect().lexer();
    let tokens = lexer
        .lex(&tables, StringOrTemplate::String(source))
        .map_or(Vec::new(), |(tokens, _)| tokens);
    let tables = Tables::default();
    parser.parse(&tables, &tokens, None).unwrap().unwrap()
}
