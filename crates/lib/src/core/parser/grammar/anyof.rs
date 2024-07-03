use std::sync::Arc;

use ahash::{AHashMap, AHashSet};
use itertools::{chain, Itertools};

use super::base::longest_trimmed_match;
use super::sequence::{Bracketed, Sequence};
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::trim_non_code_segments;
use crate::core::parser::match_algorithms::greedy_match;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment, UnparsableSegment};
use crate::core::parser::types::ParseMode;
use crate::helpers::{next_cache_key, ToErasedSegment, ToMatchable};

fn parse_mode_match_result(
    matched_segments: Vec<ErasedSegment>,
    unmatched_segments: Vec<ErasedSegment>,
    tail: Vec<ErasedSegment>,
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

    let trim_idx = unmatched_segments.iter().position(|s| s.is_code()).unwrap_or(0);

    // Create an unmatched segment
    let _expected = if let Some(first_tail_segment) = tail.first() {
        format!("Nothing else before {first_tail_segment:?}")
    } else {
        "Nothing else".to_string()
    };

    let unmatched_seg = UnparsableSegment::new(unmatched_segments[trim_idx..].to_vec());
    let mut matched = matched_segments;
    matched.extend_from_slice(&unmatched_segments[..trim_idx]);
    matched.push(unmatched_seg.to_erased_segment());

    MatchResult::new(matched, tail)
}

pub fn simple(
    elements: &[Arc<dyn Matchable>],
    parse_context: &ParseContext,
    crumbs: Option<Vec<&str>>,
) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
    let option_simples: Vec<Option<(AHashSet<String>, _)>> =
        elements.iter().map(|opt| opt.simple(parse_context, crumbs.clone())).collect();

    if option_simples.iter().any(Option::is_none) {
        return None;
    }

    let simple_buff: Vec<(AHashSet<_>, AHashSet<_>)> =
        option_simples.into_iter().flatten().collect();

    let simple_raws: AHashSet<_> = simple_buff.iter().flat_map(|(raws, _)| raws).cloned().collect();

    let simple_types: AHashSet<_> =
        simple_buff.iter().flat_map(|(_, types)| types).cloned().collect();

    Some((simple_raws, simple_types))
}

#[derive(Debug, Clone)]
#[allow(clippy::field_reassign_with_default)]
pub struct AnyNumberOf {
    pub(crate) exclude: Option<Arc<dyn Matchable>>,
    pub(crate) elements: Vec<Arc<dyn Matchable>>,
    pub(crate) terminators: Vec<Arc<dyn Matchable>>,
    pub(crate) max_times: Option<usize>,
    pub(crate) min_times: usize,
    pub(crate) max_times_per_element: Option<usize>,
    pub(crate) allow_gaps: bool,
    pub(crate) optional: bool,
    pub(crate) parse_mode: ParseMode,
    cache_key: u32,
}

impl PartialEq for AnyNumberOf {
    fn eq(&self, other: &Self) -> bool {
        self.elements.iter().zip(&other.elements).all(|(lhs, rhs)| lhs.dyn_eq(rhs.as_ref()))
    }
}

impl AnyNumberOf {
    pub fn new(elements: Vec<Arc<dyn Matchable>>) -> Self {
        Self {
            elements,
            exclude: None,
            max_times: None,
            min_times: 0,
            max_times_per_element: None,
            allow_gaps: true,
            optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: next_cache_key(),
        }
    }

    pub fn optional(&mut self) {
        self.optional = true;
    }

    pub fn disallow_gaps(&mut self) {
        self.allow_gaps = false;
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
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<(MatchResult, Option<Arc<dyn Matchable>>), SQLParseError> {
        parse_context.deeper_match(false, &self.terminators, |ctx| {
            longest_trimmed_match(segments, self.elements.clone(), ctx, false)
        })
    }
}

impl Segment for AnyNumberOf {}

impl Matchable for AnyNumberOf {
    fn is_optional(&self) -> bool {
        self.optional || self.min_times == 0
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
        simple(&self.elements, parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if let Some(exclude) = &self.exclude {
            if exclude.match_segments(segments, parse_context)?.has_match() {
                return Ok(MatchResult::from_empty());
            }
        }

        let mut matched_segments = MatchResult::from_empty();
        let mut unmatched_segments = segments.to_vec();
        let mut tail = Vec::new();

        if self.parse_mode == ParseMode::Greedy {
            let mut terminators = self.terminators.clone();
            terminators.extend(parse_context.terminators.clone());

            let term_match = greedy_match(segments.to_vec(), parse_context, terminators, false)?;
            if term_match.has_match() {
                unmatched_segments = term_match.matched_segments;
                tail = term_match.unmatched_segments;
            }
        }

        // Keep track of the number of times each option has been matched.
        let mut n_matches = 0;
        let mut option_counter: AHashMap<_, usize> =
            self.elements.iter().map(|item| (item.cache_key(), 0)).collect();

        loop {
            if self.max_times.is_some() && Some(n_matches) >= self.max_times {
                // We've matched as many times as we can
                return Ok(parse_mode_match_result(
                    matched_segments.matched_segments,
                    unmatched_segments,
                    tail,
                    self.parse_mode,
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
                        self.parse_mode,
                    ))
                } else {
                    // We didn't meet the hurdle
                    Ok(MatchResult::from_unmatched(segments.to_vec()))
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

            if let Some(matched_option) = matched_option {
                let matched_key = matched_option.cache_key();
                if let Some(counter) = option_counter.get_mut(&matched_key) {
                    *counter += 1;

                    if let Some(max_times_per_element) = self.max_times_per_element
                        && *counter > max_times_per_element
                    {
                        return Ok(parse_mode_match_result(
                            matched_segments.matched_segments,
                            chain!(pre_seg, unmatched_segments).collect_vec(),
                            tail,
                            self.parse_mode,
                        ));
                    }
                }
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
                        self.parse_mode,
                    ))
                } else {
                    // We didn't meet the hurdle
                    Ok(parse_mode_match_result(
                        vec![],
                        chain(matched_segments.matched_segments, pre_seg)
                            .chain(unmatched_segments)
                            .collect_vec(),
                        tail,
                        self.parse_mode,
                    ))
                };
            }
        }
    }

    #[track_caller]
    fn copy(
        &self,
        insert: Option<Vec<Arc<dyn Matchable>>>,
        at: Option<usize>,
        before: Option<Arc<dyn Matchable>>,
        remove: Option<Vec<Arc<dyn Matchable>>>,
        terminators: Vec<Arc<dyn Matchable>>,
        replace_terminators: bool,
    ) -> Arc<dyn Matchable> {
        let mut new_elements = self.elements.clone();

        if let Some(insert_elements) = insert {
            if let Some(before_element) = before {
                if let Some(index) = self.elements.iter().position(|e| e.hack_eq(&before_element)) {
                    new_elements.splice(index..index, insert_elements);
                } else {
                    panic!("Element for insertion before not found");
                }
            } else if let Some(at_index) = at {
                new_elements.splice(at_index..at_index, insert_elements);
            } else {
                new_elements.extend(insert_elements);
            }
        }

        if let Some(remove_elements) = remove {
            new_elements.retain(|elem| !remove_elements.iter().any(|r| Arc::ptr_eq(elem, r)));
        }

        let mut new_grammar = self.clone();

        new_grammar.elements = new_elements;
        new_grammar.terminators = if replace_terminators {
            terminators
        } else {
            [self.terminators.clone(), terminators].concat()
        };

        Arc::new(new_grammar)
    }

    fn cache_key(&self) -> u32 {
        self.cache_key
    }
}

pub fn one_of(elements: Vec<Arc<dyn Matchable>>) -> AnyNumberOf {
    let mut matcher = AnyNumberOf::new(elements);
    matcher.max_times(1);
    matcher.min_times(1);
    matcher
}

pub fn optionally_bracketed(elements: Vec<Arc<dyn Matchable>>) -> AnyNumberOf {
    let mut args = vec![Bracketed::new(elements.clone()).to_matchable()];

    if elements.len() == 1 {
        args.extend(elements);
    } else {
        args.push(Sequence::new(elements).to_matchable());
    }

    one_of(args)
}

pub fn any_set_of(elements: Vec<Arc<dyn Matchable>>) -> AnyNumberOf {
    let mut any_number_of = AnyNumberOf::new(elements);
    any_number_of.max_times_per_element = Some(1);
    any_number_of
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use serde_json::{json, Value};

    use super::{one_of, AnyNumberOf};
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::{RegexParser, StringParser};
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::test_functions::{
        fresh_ansi_dialect, generate_test_segments_func, test_segments,
    };
    use crate::core::parser::types::ParseMode;
    use crate::helpers::{ToErasedSegment, ToMatchable};

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
                        segment.raw().into(),
                        segment.get_position_marker().unwrap().into(),
                    )
                    .to_erased_segment()
                },
                None,
                false,
                None,
            );

            let fs = StringParser::new(
                "foo",
                |segment| {
                    KeywordSegment::new(
                        segment.raw().into(),
                        segment.get_position_marker().unwrap().into(),
                    )
                    .to_erased_segment()
                },
                None,
                false,
                None,
            );

            let mut g = one_of(vec![Arc::new(fs), Arc::new(bs)]);

            if allow_gaps {
                g.disallow_gaps();
            }

            let dialect = fresh_ansi_dialect();
            let mut ctx = ParseContext::new(&dialect, <_>::default());

            // Check directly
            let mut segments = g.match_segments(&test_segments(), &mut ctx).unwrap();

            assert_eq!(segments.len(), 1);
            assert_eq!(segments.matched_segments.pop().unwrap().raw(), "bar");

            // Check with a bit of whitespace
            assert!(!g.match_segments(&test_segments()[1..], &mut ctx).unwrap().has_match());
        }
    }

    #[test]
    fn test__parser__grammar_oneof_templated() {
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let fs = StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let g = one_of(vec![Arc::new(bs), Arc::new(fs)]);

        assert!(!g.match_segments(&test_segments()[5..], &mut ctx).unwrap().has_match());
    }

    #[test]
    fn test__parser__grammar_anyof_modes() {
        let cases: [(_, &[_], &[_], Value, Option<usize>); 7] = [
            // #####
            // Strict matches
            // #####
            // 1. Match once
            (ParseMode::Strict, &["a"], &[], json!([{"keyword": "a"}]), None),
            // 2. Match none
            (ParseMode::Strict, &["b"], &[], json!([]), None),
            // 3. Match twice
            (
                ParseMode::Strict,
                &["b", "a"],
                &[],
                json!([{"keyword": "a"}, {"whitespace": " "},{"keyword": "b"}]),
                None,
            ),
            // 4. Limited match
            (ParseMode::Strict, &["b", "a"], &[], json!([{"keyword": "a"}]), 1.into()),
            // #####
            // Greedy matches
            // #####
            // 1. Terminated match
            (ParseMode::Greedy, &["b", "a"], &["b"], json!([{"keyword": "a"}]), None),
            // 2. Terminated, but not matching the first element.
            (ParseMode::Greedy, &["b"], &["b"], json!([{"unparsable": [{"": "a"}]}]), None),
            // 3. Terminated, but only a partial match.
            (
                ParseMode::Greedy,
                &["a"],
                &["c"],
                json!([{"keyword": "a"}, {"whitespace": " "}, {"unparsable": [{"": "b"}]}]),
                None,
            ),
        ];

        let segments = generate_test_segments_func(vec!["a", " ", "b", " ", "c", "d", " ", "d"]);
        let dialect = fresh_ansi_dialect();
        let mut parse_cx = ParseContext::new(&dialect, <_>::default());

        for (mode, sequence, terminators, output, max_times) in cases {
            let elements = sequence
                .iter()
                .map(|it| {
                    StringParser::new(
                        it,
                        |it| {
                            KeywordSegment::new(
                                it.raw().into(),
                                it.get_position_marker().unwrap().into(),
                            )
                            .to_erased_segment()
                        },
                        None,
                        false,
                        None,
                    )
                    .to_matchable()
                })
                .collect_vec();

            let terms = terminators
                .iter()
                .map(|it| {
                    Arc::new(StringParser::new(
                        it,
                        |segment| {
                            KeywordSegment::new(
                                segment.raw().into(),
                                segment.get_position_marker().unwrap().into(),
                            )
                            .to_erased_segment()
                        },
                        None,
                        false,
                        None,
                    )) as Arc<dyn Matchable>
                })
                .collect_vec();

            let mut seq = AnyNumberOf::new(elements);
            seq.parse_mode = mode;
            seq.terminators = terms;
            if let Some(max_times) = max_times {
                seq.max_times(max_times);
            }

            let match_result = seq.match_segments(&segments, &mut parse_cx).unwrap();

            let result = match_result
                .matched_segments
                .iter()
                .map(|it| it.to_serialised(false, true, false))
                .collect_vec();

            let input = serde_json::to_value(result).unwrap();
            assert_eq!(input, output);
        }
    }

    #[test]
    fn test__parser__grammar_anysetof() {
        let token_list = vec!["bar", "  \t ", "foo", "  \t ", "bar"];
        let segments = generate_test_segments_func(token_list);

        let bar = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );
        let foo = StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let ansi = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&ansi, <_>::default());
        let g = AnyNumberOf::new(vec![Arc::new(bar), Arc::new(foo)]);
        let result = g.match_segments(&segments, &mut ctx).unwrap().matched_segments;

        assert_eq!(result[0].raw(), "bar");
        assert_eq!(result[1].raw(), "  \t ");
        assert_eq!(result[2].raw(), "foo");
    }

    #[test]
    fn test__parser__grammar_oneof_take_first() {
        let segments = test_segments();

        let foo_regex = RegexParser::new(
            "fo{2}",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
            None,
        );
        let foo = StringParser::new(
            "foo",
            |segment| {
                KeywordSegment::new(
                    segment.raw().into(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let g1 = one_of(vec![Arc::new(foo_regex.clone()), Arc::new(foo.clone())]);
        let g2 = one_of(vec![Arc::new(foo), Arc::new(foo_regex)]);

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        for segment in g1.match_segments(&segments, &mut ctx).unwrap().matched_segments.iter() {
            assert_eq!(segment.raw(), "foo");
            assert_eq!(
                segment.get_position_marker().unwrap(),
                segments[2].get_position_marker().unwrap()
            );
        }

        for segment in g2.match_segments(&segments[2..], &mut ctx).unwrap().matched_segments.iter()
        {
            assert_eq!(segment.raw(), "foo");
            assert_eq!(
                segment.get_position_marker().unwrap(),
                segments[2].get_position_marker().unwrap()
            );
        }
    }
}
