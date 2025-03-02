use std::ops::{Deref, DerefMut};

use ahash::AHashSet;

use super::anyof::{AnyNumberOf, one_of};
use super::base::Ref;
use crate::dialects::syntax::SyntaxSet;
use crate::errors::SQLParseError;
use crate::helpers::ToMatchable;
use crate::parser::context::ParseContext;
use crate::parser::grammar::noncode::NonCodeMatcher;
use crate::parser::match_algorithms::{longest_match, skip_start_index_forward_to_code};
use crate::parser::match_result::MatchResult;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};
use crate::parser::segments::base::ErasedSegment;

/// Match an arbitrary number of elements separated by a delimiter.
///
/// Note that if there are multiple elements passed in that they will be treated
/// as different options of what can be delimited, rather than a sequence.
#[derive(Clone, Debug)]
pub struct Delimited {
    pub base: AnyNumberOf,
    pub allow_trailing: bool,
    pub(crate) delimiter: Matchable,
    pub min_delimiters: usize,
    optional: bool,
    cache_key: MatchableCacheKey,
}

impl Delimited {
    pub fn new(elements: Vec<Matchable>) -> Self {
        Self {
            base: one_of(elements),
            allow_trailing: false,
            delimiter: Ref::new("CommaSegment").to_matchable(),
            min_delimiters: 0,
            optional: false,
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn allow_trailing(&mut self) {
        self.allow_trailing = true;
    }

    pub fn delimiter(&mut self, delimiter: impl ToMatchable) {
        self.delimiter = delimiter.to_matchable();
    }
}

impl PartialEq for Delimited {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.allow_trailing == other.allow_trailing
        // && self.delimiter == other.delimiter
    }
}

impl MatchableTrait for Delimited {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.optional || self.base.is_optional()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        super::anyof::simple(&self.elements, parse_context, crumbs)
    }

    /// Match an arbitrary number of elements separated by a delimiter.
    ///
    /// Note that if there are multiple elements passed in that they will be
    /// treated as different options of what can be delimited, rather than a
    /// sequence.
    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let mut delimiters = 0;
        let mut seeking_delimiter = false;
        let max_idx = segments.len() as u32;
        let mut working_idx = idx;
        let mut working_match = MatchResult::empty_at(idx);
        let mut delimiter_match = None;

        let delimiter_matcher = self.delimiter.clone();

        let mut terminator_matchers = self.terminators.clone();
        terminator_matchers.extend(
            parse_context
                .terminators
                .iter()
                .filter(|&t| &delimiter_matcher != t)
                .cloned(),
        );

        let delimiter_matchers = &[self.delimiter.clone()];

        if !self.allow_gaps {
            terminator_matchers.push(NonCodeMatcher.to_matchable());
        }

        loop {
            if self.allow_gaps && working_idx > idx {
                working_idx =
                    skip_start_index_forward_to_code(segments, working_idx, segments.len() as u32);
            }

            if working_idx >= max_idx {
                break;
            }

            let (match_result, _) = parse_context.deeper_match(false, &[], |this| {
                longest_match(segments, &terminator_matchers, working_idx, this)
            })?;

            if match_result.has_match() {
                break;
            }

            let mut push_terminators: &[_] = &[];
            if !seeking_delimiter {
                push_terminators = delimiter_matchers;
            }

            let (match_result, _) =
                parse_context.deeper_match(false, push_terminators, |this| {
                    longest_match(
                        segments,
                        if seeking_delimiter {
                            delimiter_matchers
                        } else {
                            &self.elements
                        },
                        working_idx,
                        this,
                    )
                })?;

            if !match_result.has_match() {
                break;
            }

            working_idx = match_result.span.end;

            if seeking_delimiter {
                delimiter_match = match_result.into();
            } else {
                if let Some(delimiter_match) = &delimiter_match {
                    delimiters += 1;
                    working_match = working_match.append(delimiter_match);
                }
                working_match = working_match.append(match_result);
            }

            seeking_delimiter = !seeking_delimiter;
        }

        if let Some(delimiter_match) =
            delimiter_match.filter(|_delimiter_match| self.allow_trailing && !seeking_delimiter)
        {
            delimiters += 1;
            working_match = working_match.append(delimiter_match);
        }

        if delimiters < self.min_delimiters {
            return Ok(MatchResult::empty_at(idx));
        }

        Ok(working_match)
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

impl Deref for Delimited {
    type Target = AnyNumberOf;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Delimited {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
