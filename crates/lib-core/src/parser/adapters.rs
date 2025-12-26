use crate::parser::markers::PositionMarker;
use crate::parser::segments::builder::SegmentTreeBuilder;
use crate::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use crate::templaters::TemplatedFile;
use sqruff_parser_core::parser::Parser;
use sqruff_parser_core::parser::token::{Token, TokenSpan};

pub fn token_span_from_marker(marker: &PositionMarker) -> TokenSpan {
    TokenSpan::new(marker.source_slice.clone(), marker.templated_slice.clone())
}

pub fn token_from_segment(segment: &ErasedSegment) -> Token {
    let marker = segment
        .get_position_marker()
        .expect("token segment should have a position marker");
    Token::new(
        segment.get_type(),
        segment.raw().clone(),
        token_span_from_marker(marker),
    )
}

pub fn tokens_from_segments(segments: &[ErasedSegment]) -> Vec<Token> {
    segments.iter().map(token_from_segment).collect()
}

pub fn segment_from_token(
    token: &Token,
    templated_file: &TemplatedFile,
    tables: &Tables,
) -> ErasedSegment {
    let position = PositionMarker::new(
        token.span.source_range(),
        token.span.templated_range(),
        templated_file.clone(),
        None,
        None,
    );

    SegmentBuilder::token(tables.next_id(), token.raw.clone(), token.kind)
        .with_position(position)
        .finish()
}

pub fn segments_from_tokens(
    tokens: &[Token],
    templated_file: &TemplatedFile,
    tables: &Tables,
) -> Vec<ErasedSegment> {
    tokens
        .iter()
        .map(|token| segment_from_token(token, templated_file, tables))
        .collect()
}

pub fn tree_from_tokens(
    parser: &Parser,
    tokens: &[Token],
    tables: &Tables,
    templated_file: &TemplatedFile,
) -> Result<Option<ErasedSegment>, sqruff_parser_core::errors::SQLParseError> {
    let mut builder =
        SegmentTreeBuilder::new(parser.dialect().name(), tables, templated_file.clone());
    parser.parse_with_sink(tokens, &mut builder)?;
    Ok(builder.finish())
}
