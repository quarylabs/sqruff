use ahash::AHashSet;

use super::base::ErasedSegment;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::MatchResult;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct BracketedSegmentMatcher {
    cache_key: MatchableCacheKey,
}

impl BracketedSegmentMatcher {
    pub fn new() -> Self {
        Self {
            cache_key: next_matchable_cache_key(),
        }
    }
}

impl Default for BracketedSegmentMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchableTrait for BracketedSegmentMatcher {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

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
        if segments[idx as usize].get_type() == SyntaxKind::Bracketed {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}
