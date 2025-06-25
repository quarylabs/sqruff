use ahash::AHashSet;

use super::context::ParseContext;
use super::match_algorithms::skip_start_index_forward_to_code;
use super::match_result::MatchResult;
use super::matchable::{Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key};
use super::segments::ErasedSegment;
use crate::dialects::syntax::SyntaxSet;
use crate::errors::SQLParseError;

/// A matcher that excludes patterns based on lookahead.
///
/// This is useful for cases where we need to exclude a token (like "WITH")
/// only when it's followed by a specific pattern (like "(").
#[derive(Debug, Clone, PartialEq)]
pub struct LookaheadExclude {
    /// The first token to match (e.g., "WITH")
    first_token: &'static str,
    /// The lookahead token to check for (e.g., "(")
    lookahead_token: &'static str,
    /// Unique cache key for this matcher
    cache_key: MatchableCacheKey,
}

impl LookaheadExclude {
    /// Create a new LookaheadExclude matcher.
    pub fn new(first_token: &'static str, lookahead_token: &'static str) -> Self {
        Self {
            first_token,
            lookahead_token,
            cache_key: next_matchable_cache_key(),
        }
    }
}

impl MatchableTrait for LookaheadExclude {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn is_optional(&self) -> bool {
        // Exclude patterns are not optional - they either match or don't
        false
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        // LookaheadExclude doesn't have simple matching
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Check if we're at a valid position
        if idx >= segments.len() as u32 {
            return Ok(MatchResult::empty_at(idx));
        }

        // Check if current token matches first pattern (case-insensitive)
        let current_raw = segments[idx as usize].raw();
        if current_raw.eq_ignore_ascii_case(self.first_token) {
            // Look ahead for second token, skipping any whitespace
            let next_idx =
                skip_start_index_forward_to_code(segments, idx + 1, segments.len() as u32);

            if next_idx < segments.len() as u32 {
                let next_raw = segments[next_idx as usize].raw();
                if next_raw.eq_ignore_ascii_case(self.lookahead_token) {
                    // Match found - return a match to indicate this should be excluded
                    return Ok(MatchResult::from_span(idx, idx + 1));
                }
            }
        }

        // No match - don't exclude
        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}
