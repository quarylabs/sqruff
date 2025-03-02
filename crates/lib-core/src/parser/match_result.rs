use std::borrow::Cow;
use std::cmp::Ordering;

use ahash::HashMapExt;
use nohash_hasher::IntMap;

use super::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::parser::markers::PositionMarker;

fn get_point_pos_at_idx(segments: &[ErasedSegment], idx: u32) -> PositionMarker {
    let idx = idx as usize;
    if idx < segments.len() {
        segments[idx]
            .get_position_marker()
            .unwrap()
            .start_point_marker()
    } else {
        segments[idx - 1]
            .get_position_marker()
            .unwrap()
            .end_point_marker()
    }
}

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

    pub fn apply(
        self,
        tables: &Tables,
        dialect: DialectKind,
        segments: &[ErasedSegment],
    ) -> Vec<ErasedSegment> {
        enum Trigger {
            MatchResult(MatchResult),
            Meta(SyntaxKind),
        }

        let mut result_segments = Vec::new();
        let mut trigger_locs: IntMap<u32, Vec<Trigger>> =
            IntMap::with_capacity(self.insert_segments.len() + self.child_matches.len());

        for (pos, insert) in self.insert_segments {
            trigger_locs
                .entry(pos)
                .or_default()
                .push(Trigger::Meta(insert));
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
            match idx.cmp(&max_idx) {
                Ordering::Greater => {
                    result_segments.extend_from_slice(&segments[max_idx as usize..idx as usize]);
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
                        result_segments.append(&mut trigger.apply(tables, dialect, segments));
                    }
                    Trigger::Meta(meta) => {
                        let pos = get_point_pos_at_idx(segments, idx);
                        let meta = SegmentBuilder::token(tables.next_id(), "", meta)
                            .with_position(pos)
                            .finish();
                        result_segments.push(meta);
                    }
                }
            }
        }

        if max_idx < self.span.end {
            result_segments.extend_from_slice(&segments[max_idx as usize..self.span.end as usize])
        }

        let Some(matched) = self.matched else {
            return result_segments;
        };

        match matched {
            Matched::SyntaxKind(kind) => {
                vec![
                    SegmentBuilder::node(tables.next_id(), kind, dialect, result_segments)
                        .position_from_segments()
                        .finish(),
                ]
            }
            Matched::Newtype(kind) => {
                let old = result_segments.pop().unwrap();

                vec![
                    SegmentBuilder::token(old.id(), old.raw().as_ref(), kind)
                        .with_position(old.get_position_marker().unwrap().clone())
                        .finish(),
                ]
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
