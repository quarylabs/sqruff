use ahash::AHashSet;

use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
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
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        None
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}
