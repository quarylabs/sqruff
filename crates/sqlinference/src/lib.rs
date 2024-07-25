use sqruff_lib::core::linter::linter::Linter;
use sqruff_lib::core::parser::parser::Parser;
use sqruff_lib::core::parser::segments::base::ErasedSegment;
use sqruff_lib::core::templaters::base::TemplatedFile;

pub mod aggregate_functions;
pub mod columns;
pub mod infer_tests;
pub mod inference;
pub mod test;

pub fn parse_sql(parser: &Parser, source: &str) -> ErasedSegment {
    let (tokens, _) =
        Linter::lex_templated_file(TemplatedFile::from_string(source.into()), parser.config());

    let tokens = tokens.unwrap_or_default();
    parser.parse(&tokens, None, false).unwrap().unwrap()
}
