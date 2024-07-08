use std::borrow::Cow;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use ahash::AHashSet;

use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_algorithms::greedy_match;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::{next_matchable_cache_key, Matchable, MatchableCacheKey};
use crate::core::parser::segments::base::{ErasedSegment, Segment};
use crate::helpers::{capitalize, ToMatchable};

#[derive(Clone)]
pub struct Ref {
    pub(crate) reference: Cow<'static, str>,
    pub(crate) exclude: Option<Arc<dyn Matchable>>,
    terminators: Vec<Arc<dyn Matchable>>,
    reset_terminators: bool,
    allow_gaps: bool,
    optional: bool,
    cache_key: MatchableCacheKey,
    simple_cache: OnceLock<Option<(AHashSet<String>, AHashSet<&'static str>)>>,
}

impl std::fmt::Debug for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<Ref: {}{}>", self.reference, if self.is_optional() { " [opt]" } else { "" })
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

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    // Method to get the referenced element
    fn _get_elem(&self, dialect: &Dialect) -> Arc<dyn Matchable> {
        dialect.r#ref(&self.reference)
    }

    // Static method to create a Ref instance for a keyword
    pub fn keyword(keyword: &str) -> Self {
        let name = capitalize(keyword) + "KeywordSegment";
        Ref::new(name)
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

impl Segment for Ref {}

impl Matchable for Ref {
    fn is_optional(&self) -> bool {
        self.optional
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        self.simple_cache
            .get_or_init(|| {
                if let Some(ref c) = crumbs {
                    if c.contains(&self.reference.deref()) {
                        let loop_string = c.join(" -> ");
                        panic!("Self referential grammar detected: {}", loop_string);
                    }
                }

                let mut new_crumbs = crumbs.unwrap_or_default();
                new_crumbs.push(&self.reference);

                self._get_elem(parse_context.dialect()).simple(parse_context, Some(new_crumbs))
            })
            .clone()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let elem = self._get_elem(parse_context.dialect());

        if let Some(exclude) = &self.exclude {
            let ctx =
                parse_context.deeper_match(self.reset_terminators, &self.terminators, |this| {
                    if exclude
                        .match_segments(segments, idx, this)
                        .map_err(|e| dbg!(e))
                        .map_or(false, |match_result| match_result.has_match())
                    {
                        return Some(MatchResult::empty_at(idx));
                    }

                    None
                });

            if let Some(ctx) = ctx {
                return Ok(ctx);
            }
        }

        parse_context.deeper_match(self.reset_terminators, &self.terminators, |this| {
            elem.match_segments(segments, idx, this)
        })
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[derive(Clone, Debug)]
pub struct Anything {
    cache_key: MatchableCacheKey,
    terminators: Vec<Arc<dyn Matchable>>,
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
        Self { cache_key: next_matchable_cache_key(), terminators: Vec::new() }
    }

    pub fn terminators(mut self, terminators: Vec<Arc<dyn Matchable>>) -> Self {
        self.terminators = terminators;
        self
    }
}

impl Segment for Anything {}

impl Matchable for Anything {
    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if self.terminators.is_empty() && parse_context.terminators.is_empty() {
            return Ok(MatchResult::from_span(idx, segments.len() as u32));
        }

        let mut terminators = self.terminators.clone();
        terminators.extend_from_slice(&parse_context.terminators);

        greedy_match(segments, idx, parse_context, &terminators, false, true)
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

impl Segment for Nothing {}

impl Matchable for Nothing {
    fn match_segments(
        &self,
        _segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        Ok(MatchResult::empty_at(idx))
    }
}
