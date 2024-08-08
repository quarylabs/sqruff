use std::borrow::Cow;
use std::cell::OnceCell;

use ahash::AHashSet;
use itertools::Itertools;

use super::base::{pos_marker, ErasedSegment, PathStep, Segment};
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::{next_matchable_cache_key, Matchable, MatchableCacheKey};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::ToErasedSegment;

#[derive(Debug, Clone)]
pub struct BracketedSegment {
    raw: OnceCell<String>,
    pub segments: Vec<ErasedSegment>,
    pub start_bracket: Vec<ErasedSegment>,
    pub end_bracket: Vec<ErasedSegment>,
    pub pos_marker: Option<PositionMarker>,
    pub id: u32,
    descendant_type_set: OnceCell<SyntaxSet>,
    raw_segments_with_ancestors: OnceCell<Vec<(ErasedSegment, Vec<PathStep>)>>,
}

impl PartialEq for BracketedSegment {
    fn eq(&self, other: &Self) -> bool {
        self.segments.iter().zip(&other.segments).all(|(lhs, rhs)| lhs == rhs)
            && self.start_bracket == other.start_bracket
            && self.end_bracket == other.end_bracket
    }
}

impl BracketedSegment {
    pub fn new(
        segments: Vec<ErasedSegment>,
        start_bracket: Vec<ErasedSegment>,
        end_bracket: Vec<ErasedSegment>,
        hack: bool,
    ) -> Self {
        let mut this = BracketedSegment {
            segments,
            start_bracket,
            end_bracket,
            pos_marker: None,
            id: 0,
            raw: OnceCell::new(),
            descendant_type_set: OnceCell::new(),
            raw_segments_with_ancestors: OnceCell::new(),
        };
        if !hack {
            this.pos_marker = pos_marker(&this.segments).into();
        }
        this
    }
}

impl Segment for BracketedSegment {
    fn new(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        let mut this = self.clone();
        this.segments = segments;
        this.raw = OnceCell::new();
        this.pos_marker = pos_marker(&this.segments).into();
        this.raw_segments_with_ancestors = OnceCell::new();
        this.to_erased_segment()
    }

    fn raw_segments_with_ancestors(&self) -> &OnceCell<Vec<(ErasedSegment, Vec<PathStep>)>> {
        &self.raw_segments_with_ancestors
    }

    fn copy(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self {
            raw: self.raw.clone(),
            segments,
            start_bracket: self.start_bracket.clone(),
            end_bracket: self.end_bracket.clone(),
            pos_marker: self.pos_marker.clone(),
            id: self.id,
            descendant_type_set: self.descendant_type_set.clone(),
            raw_segments_with_ancestors: self.raw_segments_with_ancestors.clone(),
        }
        .to_erased_segment()
    }

    fn descendant_type_set(&self) -> &SyntaxSet {
        self.descendant_type_set.get_or_init(|| {
            let mut result_set = SyntaxSet::EMPTY;

            for seg in self.segments() {
                result_set = result_set.union(&seg.descendant_type_set().union(&seg.class_types()));
            }

            result_set
        })
    }

    fn raw(&self) -> Cow<str> {
        self.raw.get_or_init(|| self.segments().iter().map(|segment| segment.raw()).join("")).into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Bracketed
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.pos_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.pos_marker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &self.segments
    }

    fn set_segments(&mut self, segments: Vec<ErasedSegment>) {
        self.segments = segments;
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::single(SyntaxKind::Bracketed)
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct BracketedSegmentMatcher {
    cache_key: MatchableCacheKey,
}

impl BracketedSegmentMatcher {
    pub fn new() -> Self {
        Self { cache_key: next_matchable_cache_key() }
    }
}

impl Default for BracketedSegmentMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matchable for BracketedSegmentMatcher {
    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if segments[idx as usize].as_any().downcast_ref::<BracketedSegment>().is_some() {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}
