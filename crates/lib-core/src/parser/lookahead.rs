use ahash::AHashSet;

use super::matchable::{Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key};
use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;

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

    pub(crate) fn first_token(&self) -> &str {
        self.first_token
    }

    pub(crate) fn lookahead_token(&self) -> &str {
        self.lookahead_token
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
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        // LookaheadExclude doesn't have simple matching
        None
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}
