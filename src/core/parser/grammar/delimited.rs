use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};

use crate::{
    core::parser::{
        context::ParseContext, helpers::trim_non_code_segments, match_result::MatchResult,
        matchable::Matchable, segments::base::Segment,
    },
    helpers::ToMatchable,
};

use super::{
    anyof::{one_of, AnyNumberOf},
    base::{longest_trimmed_match, Ref},
};

/// Match an arbitrary number of elements separated by a delimiter.
///
/// Note that if there are multiple elements passed in that they will be treated
/// as different options of what can be delimited, rather than a sequence.
#[derive(Clone, Debug)]
pub struct Delimited {
    base: AnyNumberOf,
    allow_trailing: bool,
    delimiter: Box<dyn Matchable>,
}

impl Delimited {
    pub fn new(elements: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            base: one_of(elements),
            allow_trailing: false,
            delimiter: Ref::new("CommaSegment".into(), None, Vec::new(), false, true, false)
                .to_matchable(),
        }
    }

    pub fn allow_trailing(&mut self, allow_trailing: bool) {
        self.allow_trailing = allow_trailing;
    }
}

impl PartialEq for Delimited {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.allow_trailing == other.allow_trailing
        // && self.delimiter == other.delimiter
    }
}

impl Matchable for Delimited {
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

    /// Match an arbitrary number of elements separated by a delimiter.
    ///
    /// Note that if there are multiple elements passed in that they will be treated
    /// as different options of what can be delimited, rather than a sequence.
    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> MatchResult {
        // Have we been passed an empty list?
        if segments.is_empty() {
            return MatchResult::from_empty();
        }

        // Make some buffers
        let mut seg_buff = segments.clone();
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
        let terminator_matchers = Vec::new();

        let mut tmp_sg = Vec::new();
        loop {
            tmp_sg = seg_buff.clone();

            if seg_buff.is_empty() {
                break;
            }

            let (pre_non_code, seg_content, post_non_code) = trim_non_code_segments(&tmp_sg);

            if !self.allow_gaps && pre_non_code.iter().any(|seg| seg.is_whitespace()) {
                unimplemented!()
            }

            if seg_content.is_empty() {
                unimplemented!()
            }

            // Check whether there is a terminator before checking for content
            let (match_result, _) =
                parse_context.deeper_match("Delimited-Term", false, &[], None, |this| {
                    longest_trimmed_match(&seg_content, terminator_matchers.clone(), this, false)
                });

            if match_result.has_match() {
                terminated = true;
                unmatched_segments = {
                    let mut segments = pre_non_code.to_vec();
                    segments.extend(match_result.all_segments());
                    segments.extend(post_non_code.to_vec());
                    segments
                };
                break;
            }

            let (match_result, _) = parse_context.deeper_match(
                "Delimited",
                false,
                if seeking_delimiter {
                    &[]
                } else {
                    &delimiter_matchers
                },
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
                        /*We've already trimmed*/ false,
                    )
                },
            );

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
                cached_matched_segments = matched_segments.clone();
                cached_unmatched_segments = seg_buff.clone();
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

            matched_segments.extend(
                pre_non_code
                    .iter()
                    .cloned()
                    .chain(match_result.matched_segments),
            );
            seeking_delimiter = !seeking_delimiter;
        }

        // if self.min_delimiters:
        //     if delimiters < self.min_delimiters:
        //         print("if delimiters < self.min_delimiters")
        //         return MatchResult.from_unmatched(matched_segments + unmatched_segments)

        if terminated {
            return if has_matched_segs {
                MatchResult {
                    matched_segments,
                    unmatched_segments,
                }
            } else {
                let mut segments = matched_segments;
                segments.extend(unmatched_segments);
                MatchResult::from_unmatched(segments)
            };
        }

        if matched_delimiter && !self.allow_trailing {
            return if unmatched_segments.is_empty() {
                let mut segments = matched_segments;
                segments.extend(unmatched_segments);
                MatchResult::from_unmatched(segments)
            } else {
                MatchResult {
                    matched_segments: cached_matched_segments,
                    unmatched_segments: cached_unmatched_segments,
                }
            };
        }

        if !has_matched_segs {
            let mut segments = matched_segments;
            segments.extend(unmatched_segments);
            return MatchResult::from_unmatched(segments);
        }

        if unmatched_segments.is_empty() {
            return MatchResult::from_matched(matched_segments);
        }

        MatchResult {
            matched_segments,
            unmatched_segments,
        }
    }

    fn cache_key(&self) -> String {
        todo!()
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
    use crate::{
        core::parser::{
            context::ParseContext,
            matchable::Matchable,
            parsers::StringParser,
            segments::{
                base::{Segment, SymbolSegment, SymbolSegmentNewArgs},
                keyword::KeywordSegment,
                test_functions::{fresh_ansi_dialect, generate_test_segments_func},
            },
        },
        helpers::ToMatchable,
        traits::Boxed,
    };

    use super::Delimited;

    #[test]
    fn test__parser__grammar_delimited() {
        let cases = [(
            /*None*/ true,
            false,
            vec!["bar", " \t ", ".", "    ", "bar"],
            5,
        )];

        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        for (allow_gaps, allow_trailing, token_list, match_len) in cases {
            let test_segments = generate_test_segments_func(token_list);
            let mut g = Delimited::new(vec![StringParser::new(
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
            .boxed()]);

            let symbol_factory = |segment: &dyn Segment| {
                SymbolSegment::new(
                    &segment.get_raw().unwrap(),
                    &segment.get_position_marker().unwrap(),
                    SymbolSegmentNewArgs {},
                )
            };
            g.delimiter = StringParser::new(".", symbol_factory, None, false, None).to_matchable();

            g.allow_gaps(allow_gaps);
            g.allow_trailing(allow_trailing);

            let match_result = g.match_segments(test_segments.clone(), &mut ctx);
            assert_eq!(match_result.len(), match_len);
        }
    }
}
