use std::borrow::Cow;
use std::cmp::Ordering;

use ahash::HashMapExt;
use nohash_hasher::IntMap;

use crate::dialects::SyntaxKind;
use crate::parser::event_sink::EventSink;
use crate::parser::token::{Token, TokenSpan};

#[derive(Debug, Clone)]
pub enum Matched {
    SyntaxKind(SyntaxKind),
    Newtype(SyntaxKind),
}

#[derive(Default, Debug, Clone)]
pub struct MatchResult {
    pub span: Span,
    pub matched: Option<Matched>,
    pub insert_segments: Vec<(u32, SyntaxKind)>,
    pub child_matches: Vec<MatchResult>,
}

impl MatchResult {
    pub fn from_span(start: u32, end: u32) -> Self {
        Self {
            span: Span { start, end },
            ..Default::default()
        }
    }

    pub fn empty_at(idx: u32) -> Self {
        Self::from_span(idx, idx)
    }

    pub fn len(&self) -> u32 {
        self.span.end - self.span.start
    }

    pub fn is_empty(&self) -> bool {
        !self.has_match()
    }

    #[allow(clippy::len_zero)]
    pub fn has_match(&self) -> bool {
        self.len() > 0 || !self.insert_segments.is_empty()
    }

    pub fn is_better_than(&self, other: &MatchResult) -> bool {
        self.len() > other.len()
    }

    pub(crate) fn append<'a>(self, other: impl Into<Cow<'a, MatchResult>>) -> Self {
        let other = other.into();
        let mut insert_segments = Vec::new();

        if self.is_empty() {
            return other.into_owned();
        }

        if other.is_empty() {
            return self;
        }

        let new_span = Span {
            start: self.span.start,
            end: other.span.end,
        };
        let mut child_matches = Vec::new();
        for mut matched in [self, other.into_owned()] {
            if matched.matched.is_some() {
                child_matches.push(matched);
            } else {
                insert_segments.append(&mut matched.insert_segments);
                child_matches.append(&mut matched.child_matches);
            }
        }

        MatchResult {
            span: new_span,
            insert_segments,
            child_matches,
            ..Default::default()
        }
    }

    pub(crate) fn wrap(self, outer_matched: Matched) -> Self {
        if self.is_empty() {
            return self;
        }

        let mut insert_segments = Vec::new();
        let span = self.span;
        let child_matches = if self.matched.is_some() {
            vec![self]
        } else {
            insert_segments = self.insert_segments;
            self.child_matches
        };

        Self {
            span,
            matched: Some(outer_matched),
            insert_segments,
            child_matches,
        }
    }

    pub fn apply_events(self, tokens: &[Token], sink: &mut impl EventSink) {
        if self.is_empty() {
            return;
        }

        let MatchResult {
            span,
            matched,
            insert_segments,
            child_matches,
        } = self;

        match matched {
            Some(Matched::SyntaxKind(kind)) => {
                sink.enter_node(kind, 0);
                emit_content(span, insert_segments, child_matches, tokens, sink);
                sink.exit_node(kind);
            }
            Some(Matched::Newtype(kind)) => {
                debug_assert!(insert_segments.is_empty() && child_matches.is_empty());
                if span.start >= span.end {
                    return;
                }
                let idx = span.end - 1;
                if let Some(token) = tokens.get(idx as usize) {
                    let new_token = Token::new(kind, token.raw.clone(), token.span.clone());
                    sink.token(&new_token);
                }
            }
            None => {
                emit_content(span, insert_segments, child_matches, tokens, sink);
            }
        }
    }
}

impl<'a> From<&'a MatchResult> for Cow<'a, MatchResult> {
    fn from(t: &'a MatchResult) -> Self {
        Cow::Borrowed(t)
    }
}

impl From<MatchResult> for Cow<'_, MatchResult> {
    fn from(t: MatchResult) -> Self {
        Cow::Owned(t)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

fn point_span_at_idx(tokens: &[Token], idx: u32) -> TokenSpan {
    if let Some(token) = tokens.get(idx as usize) {
        TokenSpan::new(
            token.span.source.start..token.span.source.start,
            token.span.templated.start..token.span.templated.start,
        )
    } else if let Some(token) = tokens.last() {
        TokenSpan::new(
            token.span.source.end..token.span.source.end,
            token.span.templated.end..token.span.templated.end,
        )
    } else {
        TokenSpan::new(0..0, 0..0)
    }
}

fn emit_token_slice(tokens: &[Token], start: u32, end: u32, sink: &mut impl EventSink) {
    if start >= end {
        return;
    }

    for token in &tokens[start as usize..end as usize] {
        sink.token(token);
    }
}

fn emit_content(
    span: Span,
    insert_segments: Vec<(u32, SyntaxKind)>,
    child_matches: Vec<MatchResult>,
    tokens: &[Token],
    sink: &mut impl EventSink,
) {
    enum Trigger {
        MatchResult(MatchResult),
        Meta(SyntaxKind),
    }

    let mut trigger_locs: IntMap<u32, Vec<Trigger>> =
        IntMap::with_capacity(insert_segments.len() + child_matches.len());

    for (pos, insert) in insert_segments {
        trigger_locs
            .entry(pos)
            .or_default()
            .push(Trigger::Meta(insert));
    }

    for match_result in child_matches {
        trigger_locs
            .entry(match_result.span.start)
            .or_default()
            .push(Trigger::MatchResult(match_result));
    }

    let mut max_idx = span.start;
    let mut keys = Vec::from_iter(trigger_locs.keys().copied());
    keys.sort();

    for idx in keys {
        match idx.cmp(&max_idx) {
            Ordering::Greater => {
                emit_token_slice(tokens, max_idx, idx, sink);
                max_idx = idx;
            }
            Ordering::Less => {
                unreachable!("This MatchResult was wrongly constructed")
            }
            Ordering::Equal => {}
        }

        for trigger in trigger_locs.remove(&idx).unwrap() {
            match trigger {
                Trigger::MatchResult(trigger) => {
                    max_idx = trigger.span.end;
                    trigger.apply_events(tokens, sink);
                }
                Trigger::Meta(meta) => {
                    let token = Token::new(meta, "", point_span_at_idx(tokens, idx));
                    sink.token(&token);
                }
            }
        }
    }

    if max_idx < span.end {
        emit_token_slice(tokens, max_idx, span.end, sink);
    }
}
