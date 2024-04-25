use std::rc::Rc;

use ahash::AHashSet;
use itertools::{chain, Itertools};
use uuid::Uuid;

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
use crate::helpers::{ToErasedSegment, ToMatchable};

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
    elements: &[Rc<dyn Matchable>],
    parse_context: &ParseContext,
    crumbs: Option<Vec<&str>>,
) -> Option<(AHashSet<String>, AHashSet<String>)> {
    let option_simples: Vec<Option<(AHashSet<String>, AHashSet<String>)>> =
        elements.iter().map(|opt| opt.simple(parse_context, crumbs.clone())).collect();

    if option_simples.iter().any(Option::is_none) {
        return None;
    }

    let simple_buff: Vec<(AHashSet<String>, AHashSet<String>)> =
        option_simples.into_iter().flatten().collect();

    let simple_raws: AHashSet<String> =
        simple_buff.iter().flat_map(|(raws, _)| raws).cloned().collect();

    let simple_types: AHashSet<String> =
        simple_buff.iter().flat_map(|(_, types)| types).cloned().collect();

    Some((simple_raws, simple_types))
}

#[derive(Debug, Clone, Hash)]
#[allow(clippy::field_reassign_with_default, clippy::derived_hash_with_manual_eq)]
pub struct AnyNumberOf {
    pub elements: Vec<Rc<dyn Matchable>>,
    pub terminators: Vec<Rc<dyn Matchable>>,
    pub max_times: Option<usize>,
    pub min_times: usize,
    pub allow_gaps: bool,
    pub optional: bool,
    pub parse_mode: ParseMode,
    cache_key: String,
}

impl PartialEq for AnyNumberOf {
    fn eq(&self, other: &Self) -> bool {
        self.elements.iter().zip(&other.elements).all(|(lhs, rhs)| lhs.dyn_eq(rhs.as_ref()))
    }
}

impl AnyNumberOf {
    pub fn new(elements: Vec<Rc<dyn Matchable>>) -> Self {
        Self {
            elements,
            max_times: None,
            min_times: 0,
            allow_gaps: true,
            optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: Uuid::new_v4().hyphenated().to_string(),
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
    ) -> Result<(MatchResult, Option<Rc<dyn Matchable>>), SQLParseError> {
        let name = std::any::type_name::<Self>();

        parse_context.deeper_match(name, false, &[], None, |ctx| {
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        simple(&self.elements, parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
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

            let (match_result, _matched_option) =
                self.match_once(&unmatched_segments, parse_context)?;

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

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }
}

pub fn one_of(elements: Vec<Rc<dyn Matchable>>) -> AnyNumberOf {
    let mut matcher = AnyNumberOf::new(elements);
    matcher.max_times(1);
    matcher.min_times(1);
    matcher
}

pub fn optionally_bracketed(elements: Vec<Rc<dyn Matchable>>) -> AnyNumberOf {
    let mut args = vec![Bracketed::new(elements.clone()).to_matchable()];

    if elements.len() == 1 {
        args.extend(elements);
    } else {
        args.push(Sequence::new(elements).to_matchable());
    }

    one_of(args)
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

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
                        segment.get_raw().unwrap(),
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
                        segment.get_raw().unwrap(),
                        segment.get_position_marker().unwrap().into(),
                    )
                    .to_erased_segment()
                },
                None,
                false,
                None,
            );

            let mut g = one_of(vec![Rc::new(fs), Rc::new(bs)]);

            if allow_gaps {
                g.disallow_gaps();
            }

            let dialect = fresh_ansi_dialect();
            let mut ctx = ParseContext::new(&dialect, <_>::default());

            // Check directly
            let mut segments = g.match_segments(&test_segments(), &mut ctx).unwrap();

            assert_eq!(segments.len(), 1);
            assert_eq!(segments.matched_segments.pop().unwrap().get_raw().unwrap(), "bar");

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
                    segment.get_raw().unwrap(),
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
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let g = one_of(vec![Rc::new(bs), Rc::new(fs)]);

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
                                it.get_raw().unwrap(),
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
                    Rc::new(StringParser::new(
                        it,
                        |segment| {
                            KeywordSegment::new(
                                segment.get_raw().unwrap(),
                                segment.get_position_marker().unwrap().into(),
                            )
                            .to_erased_segment()
                        },
                        None,
                        false,
                        None,
                    )) as Rc<dyn Matchable>
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
                    segment.get_raw().unwrap(),
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
                    segment.get_raw().unwrap(),
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
        let g = AnyNumberOf::new(vec![Rc::new(bar), Rc::new(foo)]);
        let result = g.match_segments(&segments, &mut ctx).unwrap().matched_segments;

        assert_eq!(result[0].get_raw().unwrap(), "bar");
        assert_eq!(result[1].get_raw().unwrap(), "  \t ");
        assert_eq!(result[2].get_raw().unwrap(), "foo");
    }

    #[test]
    fn test__parser__grammar_oneof_take_first() {
        let segments = test_segments();

        let foo_regex = RegexParser::new(
            "fo{2}",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
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
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
                )
                .to_erased_segment()
            },
            None,
            false,
            None,
        );

        let g1 = one_of(vec![Rc::new(foo_regex.clone()), Rc::new(foo.clone())]);
        let g2 = one_of(vec![Rc::new(foo), Rc::new(foo_regex)]);

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        for segment in g1.match_segments(&segments, &mut ctx).unwrap().matched_segments.iter() {
            assert_eq!(segment.get_raw().unwrap(), "foo");
            assert_eq!(
                segment.get_position_marker().unwrap(),
                segments[2].get_position_marker().unwrap()
            );
        }

        for segment in g2.match_segments(&segments[2..], &mut ctx).unwrap().matched_segments.iter()
        {
            assert_eq!(segment.get_raw().unwrap(), "foo");
            assert_eq!(
                segment.get_position_marker().unwrap(),
                segments[2].get_position_marker().unwrap()
            );
        }
    }
}
