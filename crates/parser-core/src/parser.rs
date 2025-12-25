pub mod context;
pub mod core;
pub mod events;
pub mod grammar;
pub mod lexer;
pub mod lookahead;
pub mod match_algorithms;
pub mod match_result;
pub mod matchable;
pub mod node_matcher;
pub mod parsers;
pub mod segments;
pub mod types;

use ahash::AHashMap;

use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxKind;
use crate::errors::SQLParseError;
use crate::parser::core::{EventSink, Token};
use crate::parser::events::{EventCollector, ParseEvent, ParseEventHandler, ParseEventHandlerSink};
use crate::parser::matchable::MatchableTrait;
use context::ParseContext;

#[derive(Clone)]
pub struct Parser<'a> {
    dialect: &'a Dialect,
    pub(crate) indentation_config: AHashMap<String, bool>,
}

impl<'a> From<&'a Dialect> for Parser<'a> {
    fn from(value: &'a Dialect) -> Self {
        Self {
            dialect: value,
            indentation_config: AHashMap::new(),
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: AHashMap<String, bool>) -> Self {
        Self {
            dialect,
            indentation_config,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        self.dialect
    }

    pub fn indentation_config(&self) -> &AHashMap<String, bool> {
        &self.indentation_config
    }

    pub fn parse_with_sink(
        &self,
        tokens: &[Token],
        sink: &mut impl EventSink,
    ) -> Result<(), SQLParseError> {
        if tokens.is_empty() {
            return Ok(());
        }

        let mut parse_cx: ParseContext = self.into();
        root_parse_events(tokens, &mut parse_cx, sink)
    }

    pub fn parse_with(
        &self,
        tokens: &[Token],
        handler: &mut impl ParseEventHandler,
    ) -> Result<(), SQLParseError> {
        let mut sink = ParseEventHandlerSink::new(handler);
        self.parse_with_sink(tokens, &mut sink)
    }

    pub fn parse_events(&self, tokens: &[Token]) -> Result<Vec<ParseEvent>, SQLParseError> {
        let mut collector = EventCollector::default();
        self.parse_with_sink(tokens, &mut collector)?;
        Ok(collector.into_events())
    }
}

fn root_parse_events(
    tokens: &[Token],
    parse_context: &mut ParseContext,
    sink: &mut impl EventSink,
) -> Result<(), SQLParseError> {
    let start_idx = tokens
        .iter()
        .position(|token| token.is_code())
        .unwrap_or(0) as u32;

    let end_idx = tokens
        .iter()
        .rposition(|token| token.is_code())
        .map_or(start_idx, |idx| idx as u32 + 1);

    sink.enter_node(SyntaxKind::File);
    emit_tokens(&tokens[..start_idx as usize], sink);

    if start_idx == end_idx {
        emit_tokens(&tokens[start_idx as usize..], sink);
        sink.exit_node(SyntaxKind::File);
        return Ok(());
    }

    let file_segment = parse_context.dialect().r#ref("FileSegment");

    let match_result = file_segment
        .match_grammar(parse_context.dialect())
        .unwrap()
        .match_segments(&tokens[..end_idx as usize], start_idx, parse_context)?;

    let match_span = match_result.span;
    let has_match = match_result.has_match();

    if !has_match {
        emit_unparsable(&tokens[start_idx as usize..end_idx as usize], sink);
    } else {
        match_result.apply_events(tokens, sink);
        let unmatched = &tokens[match_span.end as usize..end_idx as usize];
        if !unmatched.is_empty() {
            let idx = unmatched
                .iter()
                .position(|token| token.is_code())
                .unwrap_or(unmatched.len());
            let (head, tail) = unmatched.split_at(idx);
            emit_tokens(head, sink);
            if !tail.is_empty() {
                emit_unparsable(tail, sink);
            }
        }
    }

    emit_tokens(&tokens[end_idx as usize..], sink);
    sink.exit_node(SyntaxKind::File);
    Ok(())
}

fn emit_tokens(tokens: &[Token], sink: &mut impl EventSink) {
    for token in tokens {
        sink.token(token.clone());
    }
}

fn emit_unparsable(tokens: &[Token], sink: &mut impl EventSink) {
    if tokens.is_empty() {
        return;
    }
    sink.enter_node(SyntaxKind::Unparsable);
    emit_tokens(tokens, sink);
    sink.exit_node(SyntaxKind::Unparsable);
}
