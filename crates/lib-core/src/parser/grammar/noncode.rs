use ahash::AHashSet;

use crate::dialects::syntax::SyntaxSet;
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::{MatchResult, Span};
use crate::parser::matchable::{Matchable, MatchableCacheKey, MatchableTrait};
use crate::parser::segments::base::ErasedSegment;

#[derive(Debug, Clone, PartialEq)]
pub struct NonCodeMatcher;

impl MatchableTrait for NonCodeMatcher {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn is_optional(&self) -> bool {
        // Not optional
        false
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
        let mut matched_idx = idx;

        for i in idx..segments.len() as u32 {
            if segments[i as usize].is_code() {
                matched_idx = i;
                break;
            }
        }

        if matched_idx > idx {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: matched_idx,
                },
                ..Default::default()
            });
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        0
    }
}
