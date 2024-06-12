use ahash::AHashMap;

use super::segments::base::ErasedSegment;
use crate::core::parser::matchable::Matchable;
use crate::dialects::ansi::{self, Node};
use crate::helpers::ToErasedSegment;

#[derive(Debug, Clone)]
pub enum Matched {
    SyntaxKind(SyntaxKind),
    ErasedSegment(ErasedSegment),
}

#[derive(Debug, Clone)]
pub enum SyntaxKind {
    SelectClauseModifierSegment,
    WildcardIdentifierSegment,
    WildcardExpressionSegment,
    SelectClauseElementSegment,
    SelectClauseSegment,
    FunctionNameSegment,
    FunctionSegment,
    ObjectReferenceSegment,
    TableReferenceSegment,
    TableExpressionSegment,
    SamplingExpressionSegment,
    FromExpressionElementSegment,
    JoinClauseSegment,
    FromExpressionSegment,
    FromClauseSegment,
    ArrayTypeSegment,
    TypedArrayLiteralSegment,
    StructTypeSegment,
    TypedStructLiteralSegment,
    ArrayExpressionSegment,
    ColumnReferenceSegment,
    BracketedArguments,
    DatatypeSegment,
    LocalAliasSegment,
    ShorthandCastSegment,
    ExpressionSegment,
    WhereClauseSegment,
    UnorderedSelectStatementSegment,
    SetExpressionSegment,
    SelectStatementSegment,
    StatementSegment,
    FileSegment,

    Skip,
}

#[derive(Default, Debug, Clone)]
pub struct MatchResult {
    pub span: Span,
    pub matched: Option<Matched>,
    pub insert_segments: Vec<(u32, ErasedSegment)>,
    pub child_matches: Vec<MatchResult>,
}

impl MatchResult {
    pub fn from_span(start: u32, end: u32) -> Self {
        Self { span: Span { start, end }, ..Default::default() }
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

    pub fn has_match(&self) -> bool {
        self.len() > 0 || !self.insert_segments.is_empty()
    }

    pub fn is_better_than(&self, other: &MatchResult) -> bool {
        self.len() > other.len()
    }

    pub(crate) fn append(self, other: &MatchResult) -> Self {
        let mut insert_segments = Vec::new();

        if self.len() == 0 && self.insert_segments.is_empty() {
            return other.clone();
        }

        if other.len() == 0 && other.insert_segments.is_empty() {
            return self;
        }

        let new_span = Span { start: self.span.start, end: other.span.end };
        let mut child_matches = Vec::new();
        for mut matched in [self, other.clone()] {
            if matched.matched.is_some() {
                child_matches.push(matched);
            } else {
                insert_segments.append(&mut matched.insert_segments);
                child_matches.append(&mut matched.child_matches);
            }
        }

        MatchResult { span: new_span, insert_segments, child_matches, ..Default::default() }
    }

    pub(crate) fn wrap(self, outer_class: Matched) -> Self {
        if (self.span.end - self.span.start) == 0 && self.insert_segments.is_empty() {
            return self;
        }

        let span = self.span;
        let child_matches = if self.matched.is_some() { vec![self] } else { self.child_matches };

        Self { span, matched: Some(outer_class), insert_segments: Vec::new(), child_matches }
    }

    pub fn apply(self, segments: &[ErasedSegment]) -> Vec<ErasedSegment> {
        enum Trigger {
            MatchResult(MatchResult),
            Meta(ErasedSegment),
        }

        let mut result_segments = Vec::new();

        let mut trigger_locs: AHashMap<u32, Vec<Trigger>> =
            AHashMap::with_capacity(self.insert_segments.len() + self.child_matches.len());

        for (pos, insert) in self.insert_segments {
            trigger_locs.entry(pos).or_default().push(Trigger::Meta(insert));
        }

        for match_result in self.child_matches {
            trigger_locs
                .entry(match_result.span.start)
                .or_default()
                .push(Trigger::MatchResult(match_result));
        }

        let mut max_idx = self.span.start;
        let mut keys = Vec::from_iter(trigger_locs.keys().copied());
        keys.sort();

        for idx in keys {
            if idx > max_idx {
                result_segments.extend_from_slice(&segments[max_idx as usize..idx as usize]);
                max_idx = idx;
            } else if idx < max_idx {
                unreachable!("This MatchResult was wrongly constructed");
            }

            for trigger in trigger_locs.remove(&idx).unwrap() {
                match trigger {
                    Trigger::MatchResult(trigger) => {
                        max_idx = trigger.span.end;
                        result_segments.append(&mut trigger.apply(segments));
                    }
                    Trigger::Meta(_) => todo!(),
                }
            }
        }

        if max_idx < self.span.end {
            result_segments.extend_from_slice(&segments[max_idx as usize..self.span.end as usize])
        }

        let Some(matched) = self.matched else {
            return result_segments;
        };

        macro_rules! mk_from_kind {
            ($kind:expr, $($variant:ident),*) => {
                match $kind {
                    $(
                        SyntaxKind::$variant => Node::<ansi::$variant>::new().to_erased_segment(),
                    )*

                    _ => unimplemented!()
                }
            };
        }

        let segment = match matched {
            Matched::SyntaxKind(kind) => {
                if matches!(kind, SyntaxKind::FileSegment) {
                    return vec![ansi::FileSegment::default().mk_from_segments(result_segments)];
                }
                let mut kind = mk_from_kind!(
                    kind,
                    SelectClauseModifierSegment,
                    WildcardIdentifierSegment,
                    WildcardExpressionSegment,
                    SelectClauseElementSegment,
                    SelectClauseSegment,
                    FunctionNameSegment,
                    FunctionSegment,
                    ObjectReferenceSegment,
                    TableReferenceSegment,
                    TableExpressionSegment,
                    SamplingExpressionSegment,
                    FromExpressionElementSegment,
                    JoinClauseSegment,
                    FromExpressionSegment,
                    FromClauseSegment,
                    ArrayTypeSegment,
                    TypedArrayLiteralSegment,
                    StructTypeSegment,
                    TypedStructLiteralSegment,
                    ArrayExpressionSegment,
                    ColumnReferenceSegment,
                    BracketedArguments,
                    DatatypeSegment,
                    LocalAliasSegment,
                    ShorthandCastSegment,
                    ExpressionSegment,
                    WhereClauseSegment,
                    UnorderedSelectStatementSegment,
                    SetExpressionSegment,
                    SelectStatementSegment,
                    StatementSegment
                );
                kind.get_mut().set_segments(result_segments);
                return vec![kind];
            }
            Matched::ErasedSegment(segment) => segment,
        };

        vec![if result_segments.is_empty() { segment } else { segment.new(result_segments) }]
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}
