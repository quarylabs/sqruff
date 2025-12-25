use sqruff_parser_tree::lexer::Lexer;
use sqruff_parser_tree::parser::Parser;
use sqruff_parser_tree::parser::adapters::segments_from_tokens;
use sqruff_parser_tree::parser::segments::{ErasedSegment, Tables};
use sqruff_parser_tree::templaters::TemplatedFile;

pub mod aggregate_functions;
pub mod columns;
pub mod infer_tests;
pub mod inference;
pub mod test;

pub fn parse_sql(parser: &Parser, source: &str) -> ErasedSegment {
    let lex_tables = Tables::default();
    let lexer = Lexer::from(parser.dialect());
    let templated_file: TemplatedFile = source.into();
    let (tokens, _) = lexer.lex(templated_file.clone());
    let segments = segments_from_tokens(&tokens, &templated_file, &lex_tables);

    let parse_tables = Tables::default();
    parser.parse(&parse_tables, &segments).unwrap().unwrap()
}
