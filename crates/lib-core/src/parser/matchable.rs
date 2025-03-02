use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use ahash::AHashSet;
use enum_dispatch::enum_dispatch;

use super::context::ParseContext;
use super::grammar::anyof::AnyNumberOf;
use super::grammar::base::{Anything, Nothing, Ref};
use super::grammar::conditional::Conditional;
use super::grammar::delimited::Delimited;
use super::grammar::noncode::NonCodeMatcher;
use super::grammar::sequence::{Bracketed, Sequence};
use super::match_result::MatchResult;
use super::node_matcher::NodeMatcher;
use super::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use super::segments::base::ErasedSegment;
use super::segments::bracketed::BracketedSegmentMatcher;
use super::segments::meta::MetaSegment;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;

#[derive(Clone, Debug, PartialEq)]
pub struct Matchable {
    inner: Arc<MatchableTraitImpl>,
}

impl Deref for Matchable {
    type Target = MatchableTraitImpl;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Matchable {
    pub fn new(matchable: MatchableTraitImpl) -> Self {
        Self {
            inner: Arc::new(matchable),
        }
    }

    pub fn get_mut(&mut self) -> &mut MatchableTraitImpl {
        Arc::get_mut(&mut self.inner).unwrap()
    }

    pub fn make_mut(&mut self) -> &mut MatchableTraitImpl {
        Arc::make_mut(&mut self.inner)
    }

    pub fn as_conditional(&self) -> Option<&Conditional> {
        match self.inner.as_ref() {
            MatchableTraitImpl::Conditional(parser) => Some(parser),
            _ => None,
        }
    }

    pub fn as_indent(&self) -> Option<&MetaSegment> {
        match self.inner.as_ref() {
            MatchableTraitImpl::MetaSegment(parser) => Some(parser),
            _ => None,
        }
    }

    pub fn as_regex(&self) -> Option<&RegexParser> {
        match self.inner.as_ref() {
            MatchableTraitImpl::RegexParser(parser) => Some(parser),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<&Ref> {
        match self.inner.as_ref() {
            MatchableTraitImpl::Ref(parser) => Some(parser),
            _ => None,
        }
    }

    pub fn as_node_matcher(&mut self) -> Option<&mut NodeMatcher> {
        match Arc::make_mut(&mut self.inner) {
            MatchableTraitImpl::NodeMatcher(parser) => Some(parser),
            _ => None,
        }
    }
}

#[enum_dispatch(MatchableTrait)]
#[derive(Clone, Debug)]
pub enum MatchableTraitImpl {
    AnyNumberOf(AnyNumberOf),
    Bracketed(Bracketed),
    NodeMatcher(NodeMatcher),
    NonCodeMatcher(NonCodeMatcher),
    Nothing(Nothing),
    Ref(Ref),
    Sequence(Sequence),
    StringParser(StringParser),
    TypedParser(TypedParser),
    MetaSegment(MetaSegment),
    MultiStringParser(MultiStringParser),
    RegexParser(RegexParser),
    Delimited(Delimited),
    Anything(Anything),
    Conditional(Conditional),
    BracketedSegmentMatcher(BracketedSegmentMatcher),
}

impl PartialEq for MatchableTraitImpl {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ref(a), Self::Ref(b)) => {
                a.reference == b.reference
                    && a.optional == b.optional
                    && a.allow_gaps == b.allow_gaps
            }
            (Self::Delimited(a), Self::Delimited(b)) => {
                a.base == b.base && a.allow_trailing == b.allow_trailing
            }
            (Self::StringParser(a), Self::StringParser(b)) => a == b,
            (Self::TypedParser(a), Self::TypedParser(b)) => a == b,
            (Self::MultiStringParser(a), Self::MultiStringParser(b)) => a == b,
            _ => {
                std::mem::discriminant(self) == std::mem::discriminant(other)
                    && self.is_optional() == other.is_optional()
                    && self.elements() == other.elements()
            }
        }
    }
}

#[enum_dispatch]
pub trait MatchableTrait {
    fn get_type(&self) -> SyntaxKind {
        todo!()
    }

    fn match_grammar(&self) -> Option<Matchable> {
        None
    }

    fn elements(&self) -> &[Matchable];

    // Return whether this element is optional.
    fn is_optional(&self) -> bool {
        false
    }

    // Try to obtain a simple response from the matcher.
    // Returns a tuple of two sets of strings if simple.
    // The first is a set of uppercase raw strings which would match.
    // The second is a set of segment types that would match.
    // Returns None if not simple.
    // Note: the crumbs argument is used to detect recursion.
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let match_grammar = self.match_grammar()?;

        match_grammar.simple(parse_context, crumbs)
    }

    fn match_segments(
        &self,
        _segments: &[ErasedSegment],
        _idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        todo!();
    }

    fn cache_key(&self) -> MatchableCacheKey {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    #[track_caller]
    fn copy(
        &self,
        _insert: Option<Vec<Matchable>>,
        _at: Option<usize>,
        _before: Option<Matchable>,
        _remove: Option<Vec<Matchable>>,
        _terminators: Vec<Matchable>,
        _replace_terminators: bool,
    ) -> Matchable {
        unimplemented!("{}", std::any::type_name::<Self>())
    }
}

pub type MatchableCacheKey = u32;

pub fn next_matchable_cache_key() -> MatchableCacheKey {
    // The value 0 is reserved for NonCodeMatcher. This grammar matcher is somewhat
    // of a singleton, so we don't need a unique ID in the same way as other grammar
    // matchers.
    static ID: AtomicU32 = AtomicU32::new(1);

    ID.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .unwrap()
}
