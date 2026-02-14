use ahash::AHashSet;

use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
use crate::parser::matchable::{Matchable, MatchableCacheKey, MatchableTrait};

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
        _dialect: &Dialect,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        None
    }

    fn cache_key(&self) -> MatchableCacheKey {
        0
    }
}
