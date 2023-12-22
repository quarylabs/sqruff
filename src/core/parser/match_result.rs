use std::fmt;

use crate::core::parser::helpers::{join_segments_raw, trim_non_code_segments};
use crate::core::parser::segments::base::Segment;

#[derive(Clone)]
/// This should be the default response from any `match` method.
///
/// Primary arguments:
///         matched_segments: A tuple of the segments which have been
///             matched in this matching operation.
///         unmatched_segments: A tuple of the segments, which come after
///             the `matched_segments` which could not be matched.
#[derive(Debug)]
pub struct MatchResult {
    pub matched_segments: Vec<Box<dyn Segment>>,
    pub unmatched_segments: Vec<Box<dyn Segment>>,
}

impl MatchResult {
    pub fn new(
        matched_segments: Vec<Box<dyn Segment>>,
        unmatched_segments: Vec<Box<dyn Segment>>,
    ) -> Self {
        MatchResult { matched_segments, unmatched_segments }
    }

    /// Construct an empty `MatchResult`.
    pub fn from_empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }

    /// Construct a `MatchResult` from just unmatched segments.
    pub fn from_unmatched(segments: Vec<Box<dyn Segment>>) -> MatchResult {
        Self::new(Vec::new(), segments.to_vec())
    }

    pub fn from_matched(matched: Vec<Box<dyn Segment>>) -> MatchResult {
        MatchResult { unmatched_segments: vec![], matched_segments: matched }
    }

    /// Return the length of the match in characters, trimming whitespace.
    pub fn trimmed_matched_length(&self) -> usize {
        let (_, segs, _) = trim_non_code_segments(&self.matched_segments);
        segs.iter().map(|s| s.get_matched_length()).sum()
    }

    /// Return a tuple of all the segments, matched or otherwise.
    pub fn all_segments(&self) -> Vec<Box<dyn Segment>> {
        let mut all = self.matched_segments.clone();
        all.extend(self.unmatched_segments.clone());
        all
    }

    pub fn len(&self) -> usize {
        self.matched_segments.len()
    }

    /// Return true if everything has matched.
    ///
    ///         Note: An empty match is not a match so will return False.
    pub fn is_complete(&self) -> bool {
        self.unmatched_segments.is_empty() && !self.matched_segments.is_empty()
    }

    /// Return true if *anything* has matched.
    pub fn has_match(&self) -> bool {
        !self.matched_segments.is_empty()
    }

    /// Make a string from the raw matched segments.
    fn raw_matched(&self) -> String {
        join_segments_raw(&self.matched_segments)
    }
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = self.raw_matched();
        let content = if content.len() > 32 {
            format!("{}...{}", &content[..15], &content[content.len() - 15..])
        } else {
            content
        };
        write!(
            f,
            "<MatchResult {}/{}: {:?}>",
            self.matched_segments.len(),
            self.matched_segments.len() + self.unmatched_segments.len(),
            content
        )
    }
}

impl std::ops::Add for MatchResult {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut matched_segments = self.matched_segments;
        matched_segments.extend(other.matched_segments);
        MatchResult { matched_segments, unmatched_segments: self.unmatched_segments }
    }
}
