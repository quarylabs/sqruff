use std::{any::Any, collections::HashSet, fmt::Debug};

use dyn_clone::DynClone;
use dyn_ord::DynEq;
use itertools::Itertools;

use crate::core::errors::SQLParseError;

use super::{context::ParseContext, match_result::MatchResult, segments::base::Segment};

// Define a trait to represent the Matchable interface.
// This trait is similar to the abstract base class in Python.
pub trait Matchable: Any + Segment + DynClone + Debug + DynEq {
    fn from_segments(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Matchable> {
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
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        let Some(match_grammar) = self.match_grammar() else {
            return None;
        };

        match_grammar.simple(parse_context, crumbs)
    }

    // Match against this matcher.

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let Some(match_grammar) = self.match_grammar() else {
            unimplemented!(
                "{} has no match function implemented",
                std::any::type_name::<Self>()
            )
        };

        let match_result = match_grammar.match_segments(segments.clone(), parse_context)?;
        if match_result.has_match() {
            Ok(MatchResult {
                matched_segments: vec![self.from_segments(match_result.matched_segments)],
                unmatched_segments: match_result.unmatched_segments,
            })
        } else {
            Ok(MatchResult::from_unmatched(segments))
        }
    }

    // A method to generate a unique cache key for the matchable object.
    fn cache_key(&self) -> String {
        unimplemented!()
    }

    fn copy(
        &self,
        insert: Option<Vec<Box<dyn Matchable>>>,
        replace_terminators: bool,
        terminators: Vec<Box<dyn Matchable>>,
    ) -> Box<dyn Matchable> {
        let _ = (insert, replace_terminators, terminators);
        unimplemented!()
    }
}

dyn_clone::clone_trait_object!(Matchable);
