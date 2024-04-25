use std::rc::Rc;

use ahash::AHashSet;
use itertools::{chain, enumerate, multiunzip, Itertools};

use super::context::ParseContext;
use super::helpers::trim_non_code_segments;
use super::match_result::MatchResult;
use super::matchable::Matchable;
use super::segments::base::{ErasedSegment, Segment};
use super::segments::bracketed::BracketedSegment;
use crate::core::errors::SQLParseError;
use crate::helpers::ToErasedSegment;

pub fn first_trimmed_raw(seg: &dyn Segment) -> String {
    seg.get_raw_upper()
        .unwrap()
        .split(char::is_whitespace)
        .next()
        .map(ToString::to_string)
        .unwrap_or_default()
}

pub fn first_non_whitespace(segments: &[ErasedSegment]) -> Option<(String, AHashSet<String>)> {
    for segment in segments {
        if let Some(raw) = segment.first_non_whitespace_segment_raw_upper() {
            return Some((raw, segment.class_types()));
        }
    }

    None
}

#[derive(Debug)]
pub struct BracketInfo {
    bracket: ErasedSegment,
    segments: Vec<ErasedSegment>,
    bracket_type: String,
}

impl BracketInfo {
    fn to_segment(&self, end_bracket: Vec<ErasedSegment>) -> BracketedSegment {
        // Turn the contained segments into a bracketed segment.
        if end_bracket.len() != 1 {
            panic!("Expected end_bracket to contain exactly one element");
        }

        BracketedSegment::new(
            self.segments.clone(),
            vec![self.bracket.clone()],
            vec![end_bracket[0].clone()], // Assuming BaseSegment implements Clone
        )
    }
}

/// Use the simple matchers to prune which options to match on.
///
/// Works in the context of a grammar making choices between options
/// such as AnyOf or the content of Delimited.
pub fn prune_options(
    options: &[Rc<dyn Matchable>],
    segments: &[ErasedSegment],
    parse_context: &mut ParseContext,
) -> Vec<Rc<dyn Matchable>> {
    let mut available_options = vec![];
    let mut prune_buff = vec![];

    // Find the first code element to match against.
    let Some((first_raw, first_types)) = first_non_whitespace(segments) else {
        return options.to_vec();
    };

    for opt in options {
        let Some(simple) = opt.simple(parse_context, None) else {
            // This element is not simple, we have to do a
            // full match with it...
            available_options.push(opt.clone());
            continue;
        };

        // Otherwise we have a simple option, so let's use
        // it for pruning.
        let (simple_raws, simple_types) = simple;
        let mut matched = false;

        // We want to know if the first meaningful element of the str_buff
        // matches the option, based on either simple _raw_ matching or
        // simple _type_ matching.

        // Match Raws
        if simple_raws.contains(&first_raw) {
            // If we get here, it's matched the FIRST element of the string buffer.
            available_options.push(opt.clone());
            matched = true;
        }

        if !matched && !first_types.intersection(&simple_types).collect_vec().is_empty() {
            available_options.push(opt.clone());
            matched = true;
        }

        if !matched {
            prune_buff.push(opt.clone());
        }
    }

    available_options
}

// Look ahead for matches beyond the first element of the segments list.
// This function also contains the performance improved hash-matching approach
// to searching for matches, which should significantly improve performance.
// Prioritise the first match, and if multiple match at the same point the
// longest. If two matches of the same length match at the same time, then it's
// the first in the iterable of matchers.
// Returns:
//  `tuple` of (unmatched_segments, match_object, matcher).
pub fn look_ahead_match(
    segments: &[ErasedSegment],
    matchers: Vec<Rc<dyn Matchable>>,
    parse_context: &mut ParseContext,
) -> Result<(Vec<ErasedSegment>, MatchResult, Option<Rc<dyn Matchable>>), SQLParseError> {
    // Have we been passed an empty tuple?
    if segments.is_empty() {
        return Ok((Vec::new(), MatchResult::from_empty(), None));
    }

    // Here we enable a performance optimisation. Most of the time in this cycle
    // happens in loops looking for simple matchers which we should
    // be able to find a shortcut for.
    // let best_simple_match = None;
    let mut best_simple_match = None;
    let mut simple_match = None;

    for (idx, seg) in enumerate(segments) {
        let seg: &dyn Segment = &*seg.clone();
        let trimmed_seg = first_trimmed_raw(seg);
        for matcher in &matchers {
            let Some((simple_raws, simple_types)) = matcher.simple(parse_context, None) else {
                panic!(
                    "All matchers passed to `look_ahead_match()` are assumed to have a \
                     functioning `simple()` option. In a future release it will be compulsory for \
                     _all_ matchables to implement `simple()`. Please report this as a bug on \
                     GitHub along with your current query and dialect.\nProblematic matcher: \
                     {matcher:?}"
                );
            };

            assert!(
                !simple_raws.is_empty() || !simple_types.is_empty(),
                "Both simple_raws and simple_types are empty"
            );

            if !simple_raws.is_empty() && simple_raws.contains(&trimmed_seg) {
                simple_match = Some(matcher);
            }

            if !simple_types.is_empty() && simple_match.is_none() {
                unimplemented!()
                // let intersection: AHashSet<_> =
                //     simple_types.intersection(&seg.class_types).collect();
                // if !intersection.is_empty() {
                //     simple_match = Some(matcher);
                // }
            }

            // If we couldn't achieve a simple match, move on to the next option.
            let Some(simple_match) = simple_match else {
                continue;
            };

            let match_result = simple_match.match_segments(&segments[idx..], parse_context)?;

            if !match_result.has_match() {
                continue;
            }

            best_simple_match =
                (segments[..idx].to_vec(), match_result, Some(simple_match.clone())).into();
            // Stop looking through matchers
            break;
        }

        // If we have a valid match, stop looking through segments
        if best_simple_match.is_some() {
            break;
        }
    }

    Ok(if let Some(best_simple_match) = best_simple_match {
        best_simple_match
    } else {
        (Vec::new(), MatchResult::from_unmatched(segments.to_vec()), None)
    })
}

// Same as `look_ahead_match` but with bracket counting.
// NB: Given we depend on `look_ahead_match` we can also utilise
// the same performance optimisations which are implemented there.
// bracket_pairs_set: Allows specific segments to override the available
//     bracket pairs. See the definition of "angle_bracket_pairs" in the
//     BigQuery dialect for additional context on why this exists.
// Returns:
//    `tuple` of (unmatched_segments, match_object, matcher).
pub fn bracket_sensitive_look_ahead_match(
    segments: Vec<ErasedSegment>,
    matchers: Vec<Rc<dyn Matchable>>,
    parse_cx: &mut ParseContext,
    start_bracket: Option<Rc<dyn Matchable>>,
    end_bracket: Option<Rc<dyn Matchable>>,
    bracket_pairs_set: Option<&'static str>,
) -> Result<(Vec<ErasedSegment>, MatchResult, Option<Rc<dyn Matchable>>), SQLParseError> {
    let bracket_pairs_set = bracket_pairs_set.unwrap_or("bracket_pairs");

    // Have we been passed an empty tuple?
    if segments.is_empty() {
        return Ok((Vec::new(), MatchResult::from_unmatched(segments), None));
    }

    // Get hold of the bracket matchers from the dialect, and append them
    // to the list of matchers. We get them from the relevant set on the
    // dialect.
    let (bracket_types, start_bracket_refs, end_bracket_refs, persists): (
        Vec<_>,
        Vec<_>,
        Vec<_>,
        Vec<_>,
    ) = multiunzip(parse_cx.dialect().bracket_sets(bracket_pairs_set));

    // These are matchables, probably StringParsers.
    let mut start_brackets = start_bracket_refs
        .into_iter()
        .map(|seg_ref| parse_cx.dialect().r#ref(&seg_ref))
        .collect_vec();

    let mut end_brackets = end_bracket_refs
        .into_iter()
        .map(|seg_ref| parse_cx.dialect().r#ref(&seg_ref))
        .collect_vec();

    // Add any bracket-like things passed as arguments
    if let Some(start_bracket) = start_bracket {
        start_brackets.push(start_bracket);
    }

    if let Some(end_bracket) = end_bracket {
        end_brackets.push(end_bracket);
    }

    let bracket_matchers = chain(start_brackets.clone(), end_brackets.clone()).collect_vec();

    // Make some buffers
    let mut seg_buff = segments;
    let mut pre_seg_buff = Vec::new();
    let mut bracket_stack: Vec<BracketInfo> = Vec::new();

    loop {
        if !seg_buff.is_empty() {
            if !bracket_stack.is_empty() {
                // Yes, we're just looking for the closing bracket, or
                // another opening bracket.
                let (pre, match_result, matcher) =
                    look_ahead_match(&seg_buff, bracket_matchers.clone(), parse_cx)?;

                if match_result.has_match() {
                    // NB: We can only consider this as a nested bracket if the start
                    // and end tokens are not the same. If a matcher is both a start
                    // and end token we cannot deepen the bracket stack. In general,
                    // quoted strings are a typical example where the start and end
                    // tokens are the same. Currently, though, quoted strings are
                    // handled elsewhere in the parser, and there are no cases where
                    // *this* code has to handle identical start and end brackets.
                    // For now, consider this a small, speculative investment in a
                    // possible future requirement.
                    let matcher = &*matcher.unwrap();
                    let has_matching_start_bracket =
                        start_brackets.iter().any(|item| item.dyn_eq(matcher));
                    let has_matching_end_bracket =
                        end_brackets.iter().any(|item| item.dyn_eq(matcher));

                    if has_matching_start_bracket && !has_matching_end_bracket {
                        // Add any segments leading up to this to the previous
                        // bracket.
                        bracket_stack.last_mut().unwrap().segments.extend(pre);
                        // Add a bracket to the stack and add the matches from
                        // the segment.
                        let bracket_type_idx =
                            start_brackets.iter().position(|item| item.dyn_eq(matcher)).unwrap();
                        bracket_stack.push(BracketInfo {
                            bracket: match_result.matched_segments[0].clone(),
                            segments: match_result.matched_segments,
                            bracket_type: bracket_types[bracket_type_idx].clone(),
                        });
                        seg_buff = match_result.unmatched_segments;
                        continue;
                    } else if has_matching_end_bracket {
                        // Found an end bracket. Does its type match that of
                        // the innermost start bracket? E.g. ")" matches "(",
                        // "]" matches "[".
                        let end_type = bracket_types
                            [end_brackets.iter().position(|x| x.dyn_eq(matcher)).unwrap()]
                        .clone();
                        if let Some(last_bracket) = bracket_stack.last_mut() {
                            if last_bracket.bracket_type == end_type {
                                // Yes, the types match. So we've found a
                                // matching end bracket. Pop the stack, construct
                                // a bracketed segment and carry on.

                                // Complete the bracketed info
                                last_bracket.segments.extend(pre.iter().cloned());
                                last_bracket
                                    .segments
                                    .extend(match_result.matched_segments.iter().cloned());

                                // Construct a bracketed segment (as a tuple) if allowed.
                                let persist_bracket = persists
                                    [end_brackets.iter().position(|x| x.dyn_eq(matcher)).unwrap()];

                                let new_segments = if persist_bracket {
                                    vec![
                                        last_bracket
                                            .to_segment(match_result.matched_segments)
                                            .to_erased_segment()
                                            as ErasedSegment,
                                    ]
                                // Assuming to_segment returns a segment
                                } else {
                                    last_bracket.segments.clone()
                                };

                                // Remove the bracket set from the stack
                                bracket_stack.pop();

                                // If we're still in a bracket, add the new segments to
                                // that bracket, otherwise add them to the buffer
                                if let Some(last_bracket) = bracket_stack.last_mut() {
                                    last_bracket.segments.extend(new_segments);
                                } else {
                                    pre_seg_buff.extend(new_segments);
                                }
                                seg_buff.clone_from(&match_result.unmatched_segments);
                                continue;
                            }
                        }

                        // The types don't match. Error.
                        return Err(SQLParseError {
                            description: format!(
                                "Found unexpected end bracket!, was expecting {end_type:?}, but \
                                 got {matcher:?}"
                            ),
                            segment: None,
                        });
                    }
                } else {
                    let segment = bracket_stack.pop().unwrap().bracket;
                    return Err(SQLParseError {
                        description: "Couldn't find closing bracket for opening bracket."
                            .to_string(),
                        segment: Some(segment),
                    });
                }
            } else {
                // No, we're open to more opening brackets or the thing(s)
                // that we're otherwise looking for.
                let (pre, match_result, matcher) = look_ahead_match(
                    &seg_buff,
                    chain(matchers.clone(), bracket_matchers.clone()).collect_vec(),
                    parse_cx,
                )?;

                if !match_result.matched_segments.is_empty() {
                    let matcher_dyn: &dyn Matchable = &*matcher.clone().unwrap();
                    let has_matching_start_bracket =
                        start_brackets.iter().any(|item| item.dyn_eq(matcher_dyn));
                    let has_matching_end_bracket =
                        end_brackets.iter().any(|item| item.dyn_eq(matcher_dyn));

                    if matchers.iter().any(|it| it.dyn_eq(matcher_dyn)) {
                        // It's one of the things we were looking for!
                        // Return.
                        return Ok((
                            pre_seg_buff.into_iter().chain(pre).collect_vec(),
                            match_result,
                            matcher,
                        ));
                    } else if has_matching_start_bracket {
                        // We've found the start of a bracket segment.
                        // NB: It might not *actually* be the bracket itself,
                        // but could be some non-code element preceding it.
                        // That's actually ok.

                        // Add the bracket to the stack.
                        bracket_stack.push(BracketInfo {
                            bracket: match_result.matched_segments[0].clone(),
                            segments: match_result.matched_segments.clone(),
                            bracket_type: bracket_types[start_brackets
                                .iter()
                                .position(|x| x.dyn_eq(matcher_dyn))
                                .unwrap()]
                            .clone(),
                        });

                        // The matched element has already been added to the bracket.
                        // Add anything before it to the pre segment buffer.
                        // Reset the working buffer.
                        pre_seg_buff.extend(pre.iter().cloned());
                        seg_buff.clone_from(&match_result.unmatched_segments);
                        continue;
                    } else if has_matching_end_bracket {
                        // We've found an unexpected end bracket! This is likely
                        // because we're matching a section which should have
                        // ended. If we had a match, it
                        // would have matched by now, so this
                        // means no match.
                        // From here we'll drop out to the happy unmatched exit.
                    } else {
                        // This shouldn't happen!?
                        panic!(
                            "This shouldn't happen. Panic in _bracket_sensitive_look_ahead_match."
                        );
                    }
                }
            }
        } else if !bracket_stack.is_empty() {
            panic!("Couldn't find closing bracket for opened brackets: `{bracket_stack:?}`.",);
        }

        // This is the happy unmatched path. This occurs when:
        // - We reached the end with no open brackets.
        // - No match while outside a bracket stack.
        // - We found an unexpected end bracket before matching something
        // interesting. We return with the mutated segments so we can reuse any
        // bracket matching.
        return Ok((
            Vec::new(),
            MatchResult::from_unmatched(chain(pre_seg_buff, seg_buff).collect_vec()),
            None,
        ));
    }
}

/// Looks ahead to claim everything up to some future terminators.
pub fn greedy_match(
    segments: Vec<ErasedSegment>,
    parse_context: &mut ParseContext,
    matchers: Vec<Rc<dyn Matchable>>,
    include_terminator: bool,
) -> Result<MatchResult, SQLParseError> {
    let mut seg_buff = segments.clone();
    let mut seg_bank = Vec::new();

    loop {
        let (pre, mat, matcher) =
            parse_context.deeper_match("Greedy", false, &[], None, |this| {
                bracket_sensitive_look_ahead_match(
                    seg_buff.clone(),
                    matchers.clone(),
                    this,
                    None,
                    None,
                    None,
                )
            })?;

        if !mat.has_match() {
            // No terminator match? Return everything
            return Ok(MatchResult::from_matched(segments));
        }

        let matcher = matcher.unwrap_or_else(|| panic!("Match without matcher: {mat}"));
        let (strings, types) = matcher
            .simple(parse_context, None)
            .unwrap_or_else(|| panic!("Terminators require a simple method: {matcher:?}"));

        if strings.iter().all(|s| s.chars().all(|c| c.is_alphabetic())) && types.is_empty() {
            let mut allowable_match = false;

            if pre.is_empty() {
                allowable_match = true;
            }

            for element in pre.iter().rev() {
                if element.is_meta() {
                    continue;
                } else if element.is_type("whitespace") || element.is_type("newline") {
                    allowable_match = true;
                    break;
                } else {
                    // Found something other than metas and whitespace/newline.
                    break;
                }
            }

            if !allowable_match {
                seg_bank = chain!(seg_bank, pre, mat.matched_segments).collect_vec();
                seg_buff = mat.unmatched_segments;
                continue;
            }
        }

        if include_terminator {
            return Ok(MatchResult {
                matched_segments: seg_bank
                    .iter()
                    .chain(pre.iter())
                    .chain(mat.matched_segments.iter())
                    .cloned()
                    .collect(),
                unmatched_segments: mat.unmatched_segments.clone(),
            });
        }

        // We can't claim any non-code segments, so we trim them off the end.
        let buf = chain(seg_bank, pre).collect_vec();
        let (leading_nc, pre_seg_mid, trailing_nc) = trim_non_code_segments(&buf);

        let n = MatchResult {
            matched_segments: chain(leading_nc.to_vec(), pre_seg_mid.to_vec()).collect_vec(),
            unmatched_segments: chain(trailing_nc.to_vec(), mat.all_segments()).collect_vec(),
        };

        return Ok(n);
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use itertools::Itertools;

    use super::{bracket_sensitive_look_ahead_match, look_ahead_match};
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::StringParser;
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::test_functions::{
        bracket_segments, fresh_ansi_dialect, generate_test_segments_func, make_result_tuple,
        test_segments,
    };
    use crate::helpers::ToErasedSegment;

    #[test]
    fn test__parser__algorithms__look_ahead_match() {
        let test_segments = test_segments();
        let cases = [(["bar", "foo"].as_slice(), 0..1, "bar"), (["foo"].as_slice(), 2..3, "foo")];

        for (matcher_keywords, result_slice, winning_matcher) in cases {
            let matchers = matcher_keywords
                .iter()
                .map(|kw| {
                    Rc::new(StringParser::new(
                        kw,
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

            let winning_matcher: &dyn Matchable = &*matchers
                [matcher_keywords.iter().position(|&it| it == winning_matcher).unwrap()]
            .clone();

            let dialect = fresh_ansi_dialect();
            let mut cx = ParseContext::new(&dialect, <_>::default());
            let (_result_pre_match, result_match, result_matcher) =
                look_ahead_match(&test_segments, matchers, &mut cx).unwrap();

            assert!(result_matcher.unwrap().dyn_eq(winning_matcher));

            let expected_result =
                make_result_tuple(result_slice.into(), matcher_keywords, &test_segments);
            assert_eq!(result_match.matched_segments, expected_result);
        }
    }

    // Test the bracket_sensitive_look_ahead_match method of the BaseGrammar.
    #[test]
    fn test__parser__algorithms__bracket_sensitive_look_ahead_match() {
        let bs = Rc::new(StringParser::new(
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
        ));

        let fs = Rc::new(StringParser::new(
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
        ));

        // We need a dialect here to do bracket matching
        let dialect = fresh_ansi_dialect();
        let mut parse_cx = ParseContext::new(&dialect, <_>::default());

        // Basic version, we should find bar first
        let (pre_section, match_result, _matcher) = bracket_sensitive_look_ahead_match(
            bracket_segments(),
            vec![bs.clone(), fs.clone()],
            &mut parse_cx,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(pre_section, Vec::new());
        // matcher.unwrap().dyn_eq(&*fs);

        // NB the middle element is a match object
        assert_eq!(match_result.matched_segments[0].get_raw().unwrap(), "bar");
        assert_eq!(match_result.len(), 1);

        // Look ahead for foo, we should find the one AFTER the brackets, not the
        // on IN the brackets.
        let (pre_section, match_result, _matcher) = bracket_sensitive_look_ahead_match(
            bracket_segments(), // assuming this is a function call or a variable
            vec![fs.clone()],   // assuming fs is a variable
            &mut parse_cx,      // assuming ctx is a variable
            None,
            None,
            None,
        )
        .unwrap();

        // NB: The bracket segments will have been mutated, so we can't directly
        // compare. Make sure we've got a bracketed section in there.
        assert_eq!(pre_section.len(), 5);
        assert!(pre_section[2].is_type("bracketed"));
        assert!(pre_section[2].is_type("bracketed"));
        assert_eq!(pre_section[2].segments().len(), 4);

        // FIXME:
        // assert!(matcher.unwrap() == fs);

        // We shouldn't match the whitespace with the keyword
        assert_eq!(match_result.matched_segments[0].get_raw().unwrap(), "foo");
        assert_eq!(match_result.matched_segments.len(), 1);
    }

    #[test]
    fn test__parser__algorithms__bracket_fail_with_open_paren_close_square_mismatch() {
        // Assuming 'StringParser' and 'KeywordSegment' are defined elsewhere
        let fs = Rc::new(StringParser::new("foo", |_| unimplemented!(), None, false, None))
            as Rc<dyn Matchable>;

        // Assuming 'ParseContext' is defined elsewhere and requires a dialect
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default()); // Placeholder for dialect

        // Rust's error handling pattern using 'Result'
        let result = bracket_sensitive_look_ahead_match(
            generate_test_segments_func(vec![
                "select", " ", "*", " ", "from", "(", "foo",
                "]", // Bracket types don't match (parens vs square)
            ]),
            vec![fs],
            &mut ctx,
            None,
            None,
            None,
        )
        .unwrap_err();

        assert!(result.matches("Found unexpected end bracket!"));

        // Asserting that the result is an error and matches the expected error
        // pattern
    }

    #[test]
    fn test__parser__algorithms__bracket_fail_with_unexpected_end_bracket() {
        // Assuming 'StringParser', 'KeywordSegment', 'ParseContext', and other
        // necessary types are defined elsewhere
        let fs = Rc::new(StringParser::new("foo", |_| unimplemented!(), None, false, None));

        // Creating a ParseContext with a dialect
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default()); // Placeholder for dialect

        // Assuming the function 'bracket_sensitive_look_ahead_match' returns a Result
        // with a tuple
        let result = bracket_sensitive_look_ahead_match(
            generate_test_segments_func(vec![
                "bar", "(", // This bracket pair should be mutated
                ")", " ", ")", // This is the unmatched bracket
                " ", "foo",
            ]),
            vec![fs],
            &mut ctx,
            None,
            None,
            None,
        );

        match result {
            Ok((_, match_result, _)) => {
                // Check we don't match (even though there's a 'foo' at the end)
                assert!(!match_result.has_match());

                // Assuming we have a way to get unmatched_segments from match_result
                let segs = match_result.unmatched_segments;

                // Check the first bracket pair have been mutated
                assert_eq!(segs[1].get_raw().unwrap(), "()");
                // assert!(segs[1].is_bracketed());
                assert_eq!(segs[1].segments().len(), 2);

                // Check the trailing 'foo' hasn't been mutated
                assert_eq!(segs[5].get_raw().unwrap(), "foo");
                // assert!(!segs[5].is_keyword_segment());
            }
            Err(_) => panic!("Test failed due to an unexpected error"),
        }
    }
}
