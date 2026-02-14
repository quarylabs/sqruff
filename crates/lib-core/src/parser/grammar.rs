pub mod anyof;
pub mod conditional;
pub mod delimited;
pub mod noncode;
pub mod sequence;

use ahash::AHashSet;
use std::borrow::Cow;
use std::sync::OnceLock;

use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxSet;
use crate::helpers::ToMatchable;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};

#[derive(Clone)]
pub struct Ref {
    pub(crate) reference: Cow<'static, str>,
    pub exclude: Option<Matchable>,
    terminators: Vec<Matchable>,
    reset_terminators: bool,
    pub(crate) allow_gaps: bool,
    pub(crate) optional: bool,
    cache_key: MatchableCacheKey,
    simple_cache: OnceLock<Option<(AHashSet<String>, SyntaxSet)>>,
}

impl std::fmt::Debug for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Ref: {}{}>",
            self.reference,
            if self.is_optional() { " [opt]" } else { "" }
        )
    }
}

impl Ref {
    // Constructor function
    pub fn new(reference: impl Into<Cow<'static, str>>) -> Self {
        Ref {
            reference: reference.into(),
            exclude: None,
            terminators: Vec::new(),
            reset_terminators: false,
            allow_gaps: true,
            optional: false,
            cache_key: next_matchable_cache_key(),
            simple_cache: OnceLock::new(),
        }
    }

    pub fn exclude(mut self, exclude: impl ToMatchable) -> Self {
        self.exclude = exclude.to_matchable().into();
        self
    }

    pub fn terminators(mut self, terminators: Vec<Matchable>) -> Self {
        self.terminators = terminators;
        self
    }

    pub fn reset_terminators(mut self) -> Self {
        self.reset_terminators = true;
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub(crate) fn reference(&self) -> &str {
        &self.reference
    }

    pub(crate) fn terminators_slice(&self) -> &[Matchable] {
        &self.terminators
    }

    pub(crate) fn reset_terminators_flag(&self) -> bool {
        self.reset_terminators
    }

    // Static method to create a Ref instance for a keyword
    #[track_caller]
    pub fn keyword(keyword: impl Into<Cow<'static, str>>) -> Self {
        let keyword = keyword.into();

        debug_assert!(
            keyword.chars().all(|c| !c.is_lowercase()),
            "Keyword references must be uppercase: {keyword}",
        );

        Ref::new(keyword)
    }
}

impl PartialEq for Ref {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference
            && self.reset_terminators == other.reset_terminators
            && self.allow_gaps == other.allow_gaps
            && self.optional == other.optional
    }
}

impl Eq for Ref {}

impl MatchableTrait for Ref {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn is_optional(&self) -> bool {
        self.optional
    }

    fn simple(
        &self,
        dialect: &Dialect,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        self.simple_cache
            .get_or_init(|| {
                if let Some(ref c) = crumbs
                    && c.contains(&&*self.reference)
                {
                    let loop_string = c.join(" -> ");
                    panic!("Self referential grammar detected: {loop_string}");
                }

                let mut new_crumbs = crumbs.unwrap_or_default();
                new_crumbs.push(&self.reference);

                dialect
                    .r#ref(&self.reference)
                    .simple(dialect, Some(new_crumbs))
            })
            .clone()
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Clone, Debug)]
pub struct Anything {
    cache_key: MatchableCacheKey,
    terminators: Vec<Matchable>,
}

impl PartialEq for Anything {
    #[allow(unused_variables)]
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}

impl Default for Anything {
    fn default() -> Self {
        Self::new()
    }
}

impl Anything {
    pub fn new() -> Self {
        Self {
            cache_key: next_matchable_cache_key(),
            terminators: Vec::new(),
        }
    }

    pub fn terminators(mut self, terminators: Vec<Matchable>) -> Self {
        self.terminators = terminators;
        self
    }

    pub(crate) fn terminators_slice(&self) -> &[Matchable] {
        &self.terminators
    }
}

impl MatchableTrait for Anything {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nothing {}

impl Default for Nothing {
    fn default() -> Self {
        Self::new()
    }
}

impl Nothing {
    pub fn new() -> Self {
        Self {}
    }
}

impl MatchableTrait for Nothing {
    fn elements(&self) -> &[Matchable] {
        &[]
    }
}
