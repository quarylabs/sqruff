use sqruff_parser_core::parser::Parser;
use sqruff_parser_tree::lexer::Lexer;
use sqruff_parser_tree::parser::segments::builder::SegmentTreeBuilder;
use sqruff_parser_tree::parser::segments::{ErasedSegment, Tables};
use sqruff_parser_tree::templaters::TemplatedFile;

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
    let mut builder = SegmentTreeBuilder::new(
        parser.dialect().name(),
        &parse_tables,
        templated_file.clone(),
    );
    parser.parse_with_sink(&tokens, &mut builder).unwrap();
    builder.finish().unwrap()
}
