use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use ahash::AHashSet;

use super::anyof::{one_of, AnyNumberOf};
use super::base::{longest_trimmed_match, Ref};
use super::noncode::NonCodeMatcher;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::trim_non_code_segments;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment};
use crate::helpers::ToMatchable;

/// Match an arbitrary number of elements separated by a delimiter.
///
/// Note that if there are multiple elements passed in that they will be treated
/// as different options of what can be delimited, rather than a sequence.
#[derive(Clone, Debug, Hash)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct Delimited {
    base: AnyNumberOf,
    allow_trailing: bool,
    delimiter: Rc<dyn Matchable>,
    min_delimiters: Option<usize>,
    optional: bool,
    cache_key: String,
}

impl Delimited {
    pub fn new(elements: Vec<Rc<dyn Matchable>>) -> Self {
        Self {
            base: one_of(elements),
            allow_trailing: false,
            delimiter: Ref::new("CommaSegment").to_matchable(),
            min_delimiters: None,
            optional: false,
            cache_key: uuid::Uuid::new_v4().hyphenated().to_string(),
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

impl Segment for Delimited {}

impl Matchable for Delimited {
    fn is_optional(&self) -> bool {
        self.optional || self.base.is_optional()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
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
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Have we been passed an empty list?
        if segments.is_empty() {
            return Ok(MatchResult::from_empty());
        }

        // Make some buffers
        let mut seg_buff = segments.to_vec();
        let mut matched_segments = Vec::new();
        let mut unmatched_segments = Vec::new();
        let mut cached_matched_segments = Vec::new();
        let mut cached_unmatched_segments = Vec::new();

        let mut delimiters = 0;
        let mut matched_delimiter = false;

        let mut seeking_delimiter = false;
        let mut has_matched_segs = false;
        let mut terminated = false;

        let delimiter_matchers = vec![self.delimiter.clone()];
        // NOTE: If the configured delimiter is in parse_context.terminators then
        // treat is _only_ as a delimiter and not as a terminator. This happens
        // frequently during nested comma expressions.
        let mut terminator_matchers = Vec::new();

        if !self.allow_gaps {
            terminator_matchers.push(NonCodeMatcher.to_matchable());
        }

        loop {
            if seg_buff.is_empty() {
                break;
            }

            let (pre_non_code, seg_content, post_non_code) = trim_non_code_segments(&seg_buff);

            let pre_non_code = pre_non_code.to_vec();
            let post_non_code = post_non_code.to_vec();

            if !self.allow_gaps && pre_non_code.iter().any(|seg| seg.is_whitespace()) {
                unmatched_segments = seg_buff.to_vec();
                break;
            }

            if seg_content.is_empty() {
                matched_segments.extend(pre_non_code.clone());
            }

            // Check whether there is a terminator before checking for content
            let (match_result, _) =
                parse_context.deeper_match("Delimited-Term", false, &[], None, |this| {
                    longest_trimmed_match(seg_content, terminator_matchers.clone(), this, false)
                })?;

            if match_result.has_match() {
                terminated = true;
                unmatched_segments = {
                    let mut segments = pre_non_code.clone();
                    segments.extend(match_result.all_segments());
                    segments.extend(post_non_code.to_vec());
                    segments
                };
                break;
            }

            let (match_result, _) = parse_context.deeper_match(
                "Delimited",
                false,
                if seeking_delimiter { &[] } else { &delimiter_matchers },
                None,
                |this| {
                    longest_trimmed_match(
                        seg_content,
                        if seeking_delimiter {
                            delimiter_matchers.clone()
                        } else {
                            self.elements.clone()
                        },
                        this,
                        // We've already trimmed
                        false,
                    )
                },
            )?;

            if !match_result.has_match() {
                unmatched_segments = {
                    let mut segments = pre_non_code.to_vec();
                    segments.extend(match_result.unmatched_segments);
                    segments.extend(post_non_code.to_vec());
                    segments
                };
                break;
            }

            if seeking_delimiter {
                delimiters += 1;
                matched_delimiter = true;
                cached_matched_segments.clone_from(&matched_segments);
                cached_unmatched_segments.clone_from(&seg_buff);
            } else {
                matched_delimiter = false;
            }

            has_matched_segs = true;
            seg_buff = {
                let mut segments = match_result.unmatched_segments.clone();
                segments.extend(post_non_code.to_vec());
                segments
            };

            if match_result.is_complete() {
                matched_segments.extend(pre_non_code.to_vec());
                matched_segments.extend(match_result.matched_segments);
                matched_segments.extend(post_non_code.to_vec());

                break;
            }

            matched_segments
                .extend(pre_non_code.iter().cloned().chain(match_result.matched_segments));
            seeking_delimiter = !seeking_delimiter;
        }

        if Some(delimiters) < self.min_delimiters {
            let mut matched_segments = matched_segments;
            matched_segments.extend(unmatched_segments);

            return Ok(MatchResult::from_unmatched(matched_segments));
        }

        if terminated {
            return Ok(if has_matched_segs {
                MatchResult { matched_segments, unmatched_segments }
            } else {
                let mut segments = matched_segments;
                segments.extend(unmatched_segments);
                MatchResult::from_unmatched(segments)
            });
        }

        if matched_delimiter && !self.allow_trailing {
            return Ok(if unmatched_segments.is_empty() {
                let mut segments = matched_segments;
                segments.extend(unmatched_segments);
                MatchResult::from_unmatched(segments)
            } else {
                MatchResult {
                    matched_segments: cached_matched_segments,
                    unmatched_segments: cached_unmatched_segments,
                }
            });
        }

        if !has_matched_segs {
            let mut segments = matched_segments;
            segments.extend(unmatched_segments);
            return Ok(MatchResult::from_unmatched(segments));
        }

        if unmatched_segments.is_empty() {
            return Ok(MatchResult::from_matched(matched_segments));
        }

        Ok(MatchResult { matched_segments, unmatched_segments })
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use itertools::Itertools;

    use super::Delimited;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::grammar::base::Anything;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::StringParser;
    use crate::core::parser::segments::base::{Segment, SymbolSegment, SymbolSegmentNewArgs};
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::test_functions::{
        bracket_segments, fresh_ansi_dialect, generate_test_segments_func, test_segments,
    };
    use crate::helpers::{enter_panic, ToErasedSegment, ToMatchable};

    #[test]
    fn test__parser__grammar_delimited() {
        let cases = [
            // Basic testing
            (None, true, false, vec!["bar", " \t ", ".", "    ", "bar"], 5),
            (None, true, false, vec!["bar", " \t ", ".", "    ", "bar", "    "], 6),
            // Testing allow_trailing
            (None, true, false, vec!["bar", " \t ", ".", "   "], 0),
            (None, true, true, vec!["bar", " \t ", ".", "   "], 4),
            // Testing the implications of allow_gaps
            (0.into(), true, false, vec!["bar", " \t ", ".", "    ", "bar"], 5),
            (0.into(), false, false, vec!["bar", " \t ", ".", "    ", "bar"], 1),
            (1.into(), true, false, vec!["bar", " \t ", ".", "    ", "bar"], 5),
            (1.into(), false, false, vec!["bar", " \t ", ".", "    ", "bar"], 0),
            (None, true, false, vec!["bar", ".", "bar"], 3),
            (None, false, false, vec!["bar", ".", "bar"], 3),
            (1.into(), true, false, vec!["bar", ".", "bar"], 3),
            (1.into(), false, false, vec!["bar", ".", "bar"], 3),
            // Check we still succeed with something trailing right on the end.
            (1.into(), false, false, vec!["bar", ".", "bar", "foo"], 3),
            // Check min_delimiters. There's a delimiter here, but not enough to match.
            (2.into(), true, false, vec!["bar", ".", "bar", "foo"], 0),
        ];

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        for (min_delimiters, allow_gaps, allow_trailing, token_list, match_len) in cases {
            let test_segments = generate_test_segments_func(token_list);
            let mut g = Delimited::new(vec![Rc::new(StringParser::new(
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
            ))]);

            let symbol_factory = |segment: &dyn Segment| {
                SymbolSegment::create(
                    &segment.get_raw().unwrap(),
                    &segment.get_position_marker().unwrap(),
                    SymbolSegmentNewArgs { r#type: "remove me" },
                )
            };

            g.delimiter = StringParser::new(".", symbol_factory, None, false, None).to_matchable();
            if !allow_gaps {
                g.disallow_gaps();
            }
            if allow_trailing {
                g.allow_trailing();
            }
            g.min_delimiters = min_delimiters;

            let match_result = g.match_segments(&test_segments, &mut ctx).unwrap();

            assert_eq!(match_result.len(), match_len);
        }
    }

    #[test]
    fn test__parser__grammar_anything_bracketed() {
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());
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
        let matcher = Anything::new().terminators(vec![Rc::new(foo)]);

        let match_result = matcher.match_segments(&bracket_segments(), &mut ctx).unwrap();
        assert_eq!(match_result.len(), 4);
        assert_eq!(match_result.matched_segments[2].get_type(), "bracketed");
        assert_eq!(match_result.matched_segments[2].get_raw().unwrap(), "(foo    )");
        assert_eq!(match_result.unmatched_segments.len(), 2);
    }

    #[test]
    fn test__parser__grammar_anything() {
        let cases: [(&[&str], usize); 5] = [
            // No terminators, full match.
            (&[], 6),
            // If terminate with foo - match length 1.
            (&["foo"], 1),
            // If terminate with foof - unterminated. Match everything
            (&["foof"], 6),
            // Greedy matching until the first item should return none
            (&["bar"], 0),
            // NOTE: the greedy until "baar" won't match because baar is
            // a keyword and therefore is required to have whitespace
            // before it. In the test sequence "baar" does not.
            // See `greedy_match()` for details.
            (&["baar"], 6),
        ];

        for (terminators, match_length) in cases {
            let _panic = enter_panic(terminators.join(" "));

            let dialect = fresh_ansi_dialect();
            let mut cx = ParseContext::new(&dialect, <_>::default());
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

            let result = Anything::new()
                .terminators(terms)
                .match_segments(&test_segments(), &mut cx)
                .unwrap();

            assert_eq!(result.len(), match_length);
        }
    }
}
