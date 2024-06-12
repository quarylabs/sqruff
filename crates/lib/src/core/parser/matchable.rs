use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use ahash::AHashSet;
use dyn_clone::DynClone;
use dyn_ord::DynEq;

use super::context::ParseContext;
use super::grammar::base::Ref;
use super::match_result::{MatchResult, Matched, SyntaxKind};
use super::segments::base::{ErasedSegment, Segment};
use crate::core::errors::SQLParseError;

pub trait AsAnyMut {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAnyMut for T {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait Matchable: Any + Segment + DynClone + Debug + DynEq + AsAnyMut {
    fn kind(&self) -> Option<SyntaxKind> {
        todo!("{}", std::any::type_name::<Self>())
    }

    fn mk_from_segments(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        let _ = segments;
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn hack_eq(&self, rhs: &Arc<dyn Matchable>) -> bool {
        let lhs = self.as_any().downcast_ref::<Ref>();
        let rhs = rhs.as_any().downcast_ref::<Ref>();

        lhs.zip(rhs).map_or(false, |(lhs, rhs)| lhs.reference == rhs.reference)
    }

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
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        let match_grammar = self.match_grammar()?;

        match_grammar.simple(parse_context, crumbs)
    }

    // Match against this matcher.

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if idx >= segments.len() as u32 {
            return Ok(MatchResult::empty_at(idx));
        }

        if segments[idx as usize].type_name() == self.type_name() {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        let grammar = self.match_grammar().unwrap();
        let match_result = parse_context
            .deeper_match(false, &[], |ctx| grammar.match_segments(segments, idx, ctx))?;

        Ok(match_result.wrap(match self.kind() {
            Some(kind) => {
                if matches!(kind, SyntaxKind::Skip) {
                    Matched::ErasedSegment(self.clone_box())
                } else {
                    Matched::SyntaxKind(kind)
                }
            }
            None => Matched::ErasedSegment(self.clone_box()),
        }))
    }

    // A method to generate a unique cache key for the matchable object.
    //
    // Returns none for no caching key
    fn cache_key(&self) -> u32 {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    #[track_caller]
    fn copy(
        &self,
        _insert: Option<Vec<Arc<dyn Matchable>>>,
        _at: Option<usize>,
        _before: Option<Arc<dyn Matchable>>,
        _remove: Option<Vec<Arc<dyn Matchable>>>,
        _terminators: Vec<Arc<dyn Matchable>>,
        _replace_terminators: bool,
    ) -> Arc<dyn Matchable> {
        unimplemented!()
    }
}

dyn_clone::clone_trait_object!(Matchable);
