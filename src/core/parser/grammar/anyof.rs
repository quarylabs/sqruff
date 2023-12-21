use std::collections::HashSet;

use itertools::{chain, Itertools};

use crate::{
    core::parser::matchable::Matchable,
    core::{
        errors::SQLParseError,
        parser::{
            context::ParseContext, helpers::trim_non_code_segments, match_result::MatchResult,
            segments::base::Segment, types::ParseMode,
        },
    },
    helpers::ToMatchable,
};

use super::{
    base::longest_trimmed_match,
    sequence::{Bracketed, Sequence},
};

fn parse_mode_match_result(
    matched_segments: Vec<Box<dyn Segment>>,
    unmatched_segments: Vec<Box<dyn Segment>>,
    tail: Vec<Box<dyn Segment>>,
    parse_mode: ParseMode,
) -> MatchResult {
    if let ParseMode::Strict = parse_mode {
        let mut unmatched = unmatched_segments;
        unmatched.extend(tail);
        return MatchResult::new(matched_segments, unmatched);
    }

    if unmatched_segments.is_empty() || unmatched_segments.iter().all(|s| !s.is_code()) {
        let mut unmatched = unmatched_segments;
        unmatched.extend(tail);
        return MatchResult::new(matched_segments, unmatched);
    }

    let trim_idx = unmatched_segments
        .iter()
        .position(|s| s.is_code())
        .unwrap_or(0);

    // Create an unmatched segment
    let expected = if let Some(first_tail_segment) = tail.get(0) {
        format!("Nothing else before {first_tail_segment:?}")
    } else {
        "Nothing else".to_string()
    };

    let unmatched_seg = unimplemented!();
    // let unmatched_seg = UnparsableSegment::new(&unmatched_segments[trim_idx..], expected);
    let mut matched = matched_segments;
    matched.extend_from_slice(&unmatched_segments[..trim_idx]);
    matched.push(unmatched_seg);

    MatchResult::new(matched, tail)
}

pub fn simple(
    elements: &[Box<dyn Matchable>],
    parse_context: &ParseContext,
    crumbs: Option<Vec<&str>>,
) -> Option<(HashSet<String>, HashSet<String>)> {
    let option_simples: Vec<Option<(HashSet<String>, HashSet<String>)>> = elements
        .iter()
        .map(|opt| opt.simple(parse_context, crumbs.clone()))
        .collect();

    if option_simples.iter().any(Option::is_none) {
        return None;
    }

    let simple_buff: Vec<(HashSet<String>, HashSet<String>)> =
        option_simples.into_iter().flatten().collect();

    let simple_raws: HashSet<String> = simple_buff
        .iter()
        .flat_map(|(raws, _)| raws)
        .cloned()
        .collect();

    let simple_types: HashSet<String> = simple_buff
        .iter()
        .flat_map(|(_, types)| types)
        .cloned()
        .collect();

    Some((simple_raws, simple_types))
}

#[derive(Debug, Clone)]
pub struct AnyNumberOf {
    pub elements: Vec<Box<dyn Matchable>>,
    pub max_times: Option<usize>,
    pub min_times: usize,
    pub allow_gaps: bool,
}

impl PartialEq for AnyNumberOf {
    fn eq(&self, _other: &Self) -> bool {
        unimplemented!()
    }
}

impl AnyNumberOf {
    pub fn new(elements: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            elements,
            max_times: None,
            min_times: 0,
            allow_gaps: true,
        }
    }

    pub fn allow_gaps(&mut self, allow_gaps: bool) {
        self.allow_gaps = allow_gaps;
    }

    pub fn max_times(&mut self, max_times: usize) {
        self.max_times = max_times.into();
    }

    pub fn min_times(&mut self, min_times: usize) {
        self.min_times = min_times;
    }

    // Match the forward segments against the available elements once.
    // This serves as the main body of OneOf, but also a building block
    // for AnyNumberOf.
    pub fn match_once(
        &self,
        segments: &[Box<dyn Segment>],
        parse_context: &mut ParseContext,
    ) -> Result<(MatchResult, Option<Box<dyn Matchable>>), SQLParseError> {
        let name = std::any::type_name::<Self>();

        parse_context.deeper_match(name, false, &[], None, |ctx| {
            longest_trimmed_match(segments, self.elements.clone(), ctx, false)
        })
    }
}

impl Segment for AnyNumberOf {}

impl Matchable for AnyNumberOf {
    fn is_optional(&self) -> bool {
        self.min_times == 0
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        simple(&self.elements, parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let mut matched_segments = MatchResult::from_empty();
        let mut unmatched_segments = segments.clone();
        let tail = Vec::new();

        // Keep track of the number of times each option has been matched.
        let mut n_matches = 0;
        // let option_counter = {elem.cache_key(): 0 for elem in self._elements}
        loop {
            if Some(n_matches) >= self.max_times {
                // We've matched as many times as we can
                return Ok(parse_mode_match_result(
                    matched_segments.matched_segments,
                    unmatched_segments,
                    tail,
                    ParseMode::Strict,
                ));
            }

            // Is there anything left to match?
            if unmatched_segments.is_empty() {
                return if n_matches >= self.min_times {
                    // No...
                    Ok(parse_mode_match_result(
                        matched_segments.matched_segments,
                        unmatched_segments,
                        tail,
                        ParseMode::Strict,
                    ))
                } else {
                    // We didn't meet the hurdle
                    Ok(MatchResult::from_unmatched(segments))
                };
            }

            let pre_seg = if n_matches > 0 && self.allow_gaps {
                let segments = std::mem::take(&mut unmatched_segments);
                let (pre_seg, mid_seg, post_seg) = trim_non_code_segments(&segments);

                unmatched_segments = chain(mid_seg, post_seg).cloned().collect_vec();

                pre_seg.to_vec()
            } else {
                Vec::new()
            };

            let (match_result, matched_option) =
                self.match_once(&unmatched_segments, parse_context)?;

            // Increment counter for matched option.
            if let Some(_matched_option) = matched_option {
                // TODO:
                // if matched_option.cache_key() in option_counter:
            }

            if match_result.has_match() {
                matched_segments
                    .matched_segments
                    .extend(chain(pre_seg, match_result.matched_segments));
                unmatched_segments = match_result.unmatched_segments;
                n_matches += 1;
            } else {
                // If we get here, then we've not managed to match. And the next
                // unmatched segments are meaningful, i.e. they're not what we're
                // looking for.
                return if n_matches >= self.min_times {
                    Ok(parse_mode_match_result(
                        matched_segments.matched_segments,
                        chain(pre_seg, unmatched_segments).collect_vec(),
                        tail,
                        ParseMode::Strict,
                    ))
                } else {
                    // We didn't meet the hurdle
                    Ok(parse_mode_match_result(
                        vec![],
                        chain(matched_segments.matched_segments, pre_seg)
                            .chain(unmatched_segments)
                            .collect_vec(),
                        tail,
                        ParseMode::Strict,
                    ))
                };
            }
        }
    }

    fn cache_key(&self) -> String {
        todo!()
    }
}

pub fn one_of(elements: Vec<Box<dyn Matchable>>) -> AnyNumberOf {
    let mut matcher = AnyNumberOf::new(elements);
    matcher.max_times(1);
    matcher.min_times(1);
    matcher
}

pub fn optionally_bracketed(elements: Vec<Box<dyn Matchable>>) -> AnyNumberOf {
    let mut args = vec![Bracketed::new(elements.clone()).to_matchable()];

    if elements.len() > 1 {
        args.push(Sequence::new(elements).to_matchable());
    } else {
        args.extend(elements);
    }

    one_of(args)
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::{
        core::parser::{
            context::ParseContext,
            matchable::Matchable,
            parsers::StringParser,
            segments::{
                keyword::KeywordSegment,
                test_functions::{fresh_ansi_dialect, generate_test_segments_func, test_segments},
            },
        },
        helpers::{Boxed, ToMatchable},
    };

    use super::{one_of, AnyNumberOf};

    #[test]
    fn test__parser__grammar_oneof() {
        // Test cases with allow_gaps as true and false
        let test_cases = [true, false];

        for allow_gaps in test_cases {
            // Test the OneOf grammar.
            // NOTE: Should behave the same regardless of allow_gaps.

            let bs = StringParser::new(
                "bar",
                |segment| {
                    KeywordSegment::new(
                        segment.get_raw().unwrap(),
                        segment.get_position_marker().unwrap(),
                    )
                    .boxed()
                },
                None,
                false,
                None,
            )
            .boxed();

            let fs = StringParser::new(
                "foo",
                |segment| {
                    KeywordSegment::new(
                        segment.get_raw().unwrap(),
                        segment.get_position_marker().unwrap(),
                    )
                    .boxed()
                },
                None,
                false,
                None,
            )
            .boxed();

            let mut g = one_of(vec![fs, bs]);
            g.allow_gaps(allow_gaps);

            let mut ctx = ParseContext::new(fresh_ansi_dialect());

            // Check directly
            let mut segments = g.match_segments(test_segments(), &mut ctx).unwrap();

            assert_eq!(segments.len(), 1);
            assert_eq!(
                segments.matched_segments.pop().unwrap().get_raw().unwrap(),
                "bar"
            );

            // Check with a bit of whitespace
            assert!(!g
                .match_segments(test_segments()[1..].to_vec(), &mut ctx)
                .unwrap()
                .has_match());
        }
    }

    #[test]
    fn test__parser__grammar_oneof_templated() {
        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let fs = StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let g = one_of(vec![bs, fs]);

        assert!(!g
            .match_segments(test_segments()[5..].to_vec(), &mut ctx)
            .unwrap()
            .has_match());
    }

    #[test]
    fn test__parser__grammar_anyof_modes() {
        let cases: [(&[_], &[_]); 3] = [
            (&["a"], &[("a", "kw")]),
            (&["b"], &[]),
            (
                &["b", "a"],
                &[("a", "kw"), (" ", "whitespace"), ("b", "kw")],
            ),
        ];

        let segments = generate_test_segments_func(vec!["a", " ", "b", " ", "c", "d", " ", "d"]);
        let mut parse_cx = ParseContext::new(fresh_ansi_dialect());

        for (sequence, output_tuple) in cases {
            let elements = sequence
                .iter()
                .map(|it| {
                    StringParser::new(
                        it,
                        |it| {
                            KeywordSegment::new(
                                it.get_raw().unwrap(),
                                it.get_position_marker().unwrap(),
                            )
                            .boxed()
                        },
                        None,
                        false,
                        None,
                    )
                    .to_matchable()
                })
                .collect_vec();

            let seq = AnyNumberOf::new(elements);

            let match_result = seq.match_segments(segments.clone(), &mut parse_cx).unwrap();
            let matched_segments = match_result.matched_segments;

            let result = matched_segments
                .into_iter()
                .map(|segment| (segment.get_raw().unwrap(), segment.get_type()))
                .collect_vec();

            let are_equal = result
                .iter()
                .map(|(s, str_ref)| (s.as_str(), str_ref))
                .zip(output_tuple.iter())
                .all(|((s1, str_ref1), (s2, str_ref2))| s1 == *s2 && str_ref1 == str_ref2);

            assert!(are_equal);
        }
    }
}
