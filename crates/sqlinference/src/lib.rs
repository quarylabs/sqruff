use sqruff_lib_core::lexer::Lexer;
use sqruff_lib_core::parser::adapters::tree_from_tokens;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};
use sqruff_lib_core::templaters::TemplatedFile;
use sqruff_parser_core::parser::Parser;

pub mod aggregate_functions;
pub mod columns;
pub mod infer_tests;
pub mod inference;
pub mod test;

pub fn parse_sql(parser: &Parser, source: &str) -> ErasedSegment {
    let lexer = Lexer::from(parser.dialect());
    let templated_file: TemplatedFile = source.into();
    let (tokens, _) = lexer.lex(&templated_file);

    let parse_tables = Tables::default();
    tree_from_tokens(parser, &tokens, &parse_tables, &templated_file)
        .unwrap()
        .unwrap()
}
