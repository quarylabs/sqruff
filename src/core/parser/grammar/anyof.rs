use std::collections::HashSet;

use itertools::{chain, Itertools};

use crate::core::parser::{
    context::ParseContext, helpers::trim_non_code_segments, match_algorithms::prune_options,
    match_result::MatchResult, matchable::Matchable, segments::base::Segment,
};

#[derive(Debug, Clone)]
pub struct AnyNumberOf {
    elements: Vec<Box<dyn Matchable>>,
    max_times: Option<usize>,
    min_times: usize,
    allow_gaps: bool,
}

impl PartialEq for AnyNumberOf {
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}

impl AnyNumberOf {
    pub fn new(elements: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            elements,
            max_times: None,
            min_times: 1,
            allow_gaps: true,
        }
    }

    fn _longest_trimmed_match(
        &self,
        segments: &[Box<dyn Segment>],
        matchers: Vec<Box<dyn Matchable>>,
        parse_context: &mut ParseContext,
        trim_noncode: bool,
    ) -> (MatchResult, Option<Box<dyn Matchable>>) {
        // Have we been passed an empty list?
        if segments.is_empty() {
            return (MatchResult::from_empty(), None);
        }
        // If presented with no options, return no match
        else if matchers.is_empty() {
            return (MatchResult::from_unmatched(segments), None);
        }

        let available_options = prune_options(&matchers, segments, parse_context);

        if available_options.is_empty() {
            return (MatchResult::from_unmatched(segments), None);
        }

        unimplemented!()
    }

    // Match the forward segments against the available elements once.
    // This serves as the main body of OneOf, but also a building block
    // for AnyNumberOf.
    pub fn match_once(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> (MatchResult, Option<Box<dyn Matchable>>) {
        let name = std::any::type_name::<Self>();

        parse_context.deeper_match(name, false, &[], None, |ctx| {
            self._longest_trimmed_match(&segments, self.elements.clone(), ctx, false)
        })
    }
}

impl Matchable for AnyNumberOf {
    fn is_optional(&self) -> bool {
        todo!()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        todo!()
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> MatchResult {
        let mut unmatched_segments = segments;

        // Keep track of the number of times each option has been matched.
        let n_matches = 0;
        // let option_counter = {elem.cache_key(): 0 for elem in self._elements}
        loop {
            if self.max_times.is_some() && Some(n_matches) >= self.max_times {
                // We've matched as many times as we can
                unimplemented!()
            }

            // Is there anything left to match?
            if unmatched_segments.is_empty() {
                unimplemented!()
            }

            let pre_seg = if n_matches > 0 && self.allow_gaps {
                let segments = std::mem::take(&mut unmatched_segments);
                let (pre_seg, mid_seg, post_seg) = trim_non_code_segments(&segments);

                unmatched_segments = chain(mid_seg, post_seg).cloned().collect_vec();

                pre_seg.to_vec()
            } else {
                Vec::new()
            };

            let (match_result, matched_option) = self.match_once(unmatched_segments, parse_context);

            dbg!(match_result.matched_segments.len());
            dbg!(match_result.unmatched_segments.len());

            unimplemented!()
        }
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{
        core::{
            dialects::init::{dialect_selector, get_default_dialect},
            parser::{
                context::ParseContext, matchable::Matchable, parsers::StringParser,
                segments::test_functions::generate_test_segments_func,
            },
        },
        helpers::ToMatchable,
    };

    use super::AnyNumberOf;

    #[test]
    fn test__parser__grammar_anyof_modes() {
        let cases = [(["a"].as_slice(),)];

        let segments = generate_test_segments_func(vec!["a", " ", "b", " ", "c", "d", " ", "d"]);
        let mut parse_cx = ParseContext::new(dialect_selector(get_default_dialect()).unwrap());

        for (sequence,) in cases {
            let elements = sequence
                .iter()
                .map(|it| {
                    StringParser::new(it, |it| unimplemented!(), None, false, None).to_matchable()
                })
                .collect_vec();

            let seq = AnyNumberOf::new(elements);

            let match_result = seq.match_segments(segments, &mut parse_cx);

            unimplemented!()
        }
    }
}
