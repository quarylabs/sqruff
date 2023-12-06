use std::{collections::HashSet, fmt::Debug};

use dyn_clone::DynClone;
use dyn_ord::DynEq;

use super::{context::ParseContext, match_result::MatchResult, segments::base::Segment};

// Define a trait to represent the Matchable interface.
// This trait is similar to the abstract base class in Python.
pub trait Matchable: DynClone + Debug + DynEq {
    // Return whether this element is optional.
    fn is_optional(&self) -> bool;

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
    ) -> Option<(HashSet<String>, HashSet<String>)>;

    // Match against this matcher.
    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> MatchResult;

    // A method to generate a unique cache key for the matchable object.
    fn cache_key(&self) -> String;

    // Copy this Matchable.
    // This method is usually used for copying Matchable objects during dialect inheritance.
    // One dialect might make a copy (usually with some modifications)
    // to a dialect element of a parent dialect which it can then use
    // itself. This provides more modularity in dialect definition.
    fn copy(&self) -> Self
    where
        Self: Sized + Clone,
    {
        self.clone()
    }
}

dyn_clone::clone_trait_object!(Matchable);
