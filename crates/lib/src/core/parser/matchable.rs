use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

use ahash::AHashSet;
use dyn_clone::DynClone;
use dyn_ord::DynEq;

use super::context::ParseContext;
use super::match_result::MatchResult;
use super::segments::base::{ErasedSegment, Segment};
use crate::core::errors::SQLParseError;

// Define a trait to represent the Matchable interface.
// This trait is similar to the abstract base class in Python.
pub trait Matchable: Any + Segment + DynClone + Debug + DynEq {
    fn mk_from_segments(&self, segments: Vec<ErasedSegment>) -> ErasedSegment {
        let _ = segments;
        unimplemented!("{}", std::any::type_name::<Self>())
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        let match_grammar = self.match_grammar()?;

        match_grammar.simple(parse_context, crumbs)
    }

    // Match against this matcher.

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let Some(match_grammar) = self.match_grammar() else {
            unimplemented!("{} has no match function implemented", std::any::type_name::<Self>())
        };

        if segments.len() == 1 && segments[0].get_type() == self.get_type() {
            return Ok(MatchResult::from_matched(segments.to_vec()));
        } else if segments.len() > 1 && segments[0].get_type() == self.get_type() {
            let (first_segment, remaining_segments) =
                segments.split_first().expect("segments should not be empty");
            return Ok(MatchResult {
                matched_segments: vec![first_segment.clone()],
                unmatched_segments: remaining_segments.to_vec(),
            });
        }

        let match_result = match_grammar.match_segments(segments, parse_context)?;

        if match_result.has_match() {
            Ok(MatchResult {
                matched_segments: vec![self.mk_from_segments(match_result.matched_segments)],
                unmatched_segments: match_result.unmatched_segments,
            })
        } else {
            Ok(MatchResult::from_unmatched(segments.to_vec()))
        }
    }

    // A method to generate a unique cache key for the matchable object.
    fn cache_key(&self) -> String {
        unimplemented!("{}", std::any::type_name::<Self>())
    }

    fn copy(
        &self,
        insert: Option<Vec<Rc<dyn Matchable>>>,
        replace_terminators: bool,
        terminators: Vec<Rc<dyn Matchable>>,
    ) -> Rc<dyn Matchable> {
        let _ = (insert, replace_terminators, terminators);
        unimplemented!()
    }
}

dyn_clone::clone_trait_object!(Matchable);
dyn_hash::hash_trait_object!(Matchable);
