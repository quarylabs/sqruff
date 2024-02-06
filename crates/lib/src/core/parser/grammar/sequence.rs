fn trim_to_terminator(
    segments: Vec<Box<dyn Segment>>,
    tail: Vec<Box<dyn Segment>>,
    terminators: Vec<Box<dyn Matchable>>,
    parse_context: &mut ParseContext,
) -> Result<(Vec<Box<dyn Segment>>, Vec<Box<dyn Segment>>), SQLParseError> {
    let term_match =
        parse_context.deeper_match("Sequence-GreedyB-@0", false, &[], false.into(), |this| {
            greedy_match(segments.clone(), this, terminators, false)
        })?;

    if term_match.has_match() {
        // If we _do_ find a terminator, we separate off everything
        // beyond that terminator (and any preceding non-code) so that
        // it's not available to match against for the rest of this.
        let tail = &term_match.unmatched_segments;
        let segments = &term_match.matched_segments;

        for (idx, segment) in segments.iter().enumerate().rev() {
            if segment.is_code() {
                return Ok(split_and_concatenate(segments, idx, tail));
            }
        }
    }

    Ok((segments.clone(), tail.clone()))
}

fn split_and_concatenate<T>(segments: &[T], idx: usize, tail: &[T]) -> (Vec<T>, Vec<T>)
where
    T: Clone,
{
    let first_part = segments[..idx + 1].to_vec();
    let second_part = segments[idx + 1..].iter().chain(tail).cloned().collect();

    (first_part, second_part)
}

fn position_metas(
    metas: &[Indent],              // Assuming Indent is a struct or type alias
    non_code: &[Box<dyn Segment>], // Assuming BaseSegment is a struct or type alias
) -> Vec<Box<dyn Segment>> {
    // Assuming BaseSegment can be cloned, or you have a way to handle ownership
    // transfer

    // Check if all metas have a non-negative indent value
    if metas.iter().all(|m| m.indent_val >= 0) {
        let mut result: Vec<Box<dyn Segment>> = Vec::new();

        // Append metas first, then non-code elements
        for meta in metas {
            result.push(meta.clone().boxed()); // Assuming clone is possible or some equivalent
        }
        for segment in non_code {
            result.push(segment.clone()); // Assuming clone is possible or some equivalent
        }

        result
    } else {
        let mut result: Vec<Box<dyn Segment>> = Vec::new();

        // Append non-code elements first, then metas
        for segment in non_code {
            result.push(segment.clone()); // Assuming clone is possible or some equivalent
        }
        for meta in metas {
            result.push(meta.clone().boxed()); // Assuming clone is possible or some equivalent
        }

        result
    }
}

use std::collections::HashSet;
use std::iter::zip;
use std::ops::{Deref, DerefMut};

use itertools::{chain, enumerate, Itertools};

use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::trim_non_code_segments;
use crate::core::parser::match_algorithms::{bracket_sensitive_look_ahead_match, greedy_match};
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{position_segments, Segment};
use crate::core::parser::segments::bracketed::BracketedSegment;
use crate::core::parser::segments::meta::Indent;
use crate::core::parser::types::ParseMode;
use crate::helpers::Boxed;

#[derive(Debug, Clone, Hash)]
pub struct Sequence {
    elements: Vec<Box<dyn Matchable>>,
    parse_mode: ParseMode,
    allow_gaps: bool,
    is_optional: bool,
    terminators: Vec<Box<dyn Matchable>>,
}

impl Sequence {
    pub fn new(elements: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            elements,
            allow_gaps: true,
            is_optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
        }
    }

    pub fn optional(&mut self) {
        self.is_optional = true;
    }

    pub fn terminators(mut self, terminators: Vec<Box<dyn Matchable>>) -> Self {
        self.terminators = terminators;
        self
    }

    pub fn parse_mode(&mut self, mode: ParseMode) {
        self.parse_mode = mode;
    }

    pub fn allow_gaps(mut self, allow_gaps: bool) -> Self {
        self.allow_gaps = allow_gaps;
        self
    }
}

impl PartialEq for Sequence {
    fn eq(&self, other: &Self) -> bool {
        zip(&self.elements, &other.elements).all(|(a, b)| a.dyn_eq(&**b))
    }
}

impl Segment for Sequence {}

impl Matchable for Sequence {
    fn is_optional(&self) -> bool {
        self.is_optional
    }

    // Does this matcher support a uppercase hash matching route?
    //
    // Sequence does provide this, as long as the *first* non-optional
    // element does, *AND* and optional elements which preceded it also do.
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        let mut simple_raws = HashSet::new();
        let mut simple_types = HashSet::new();

        for opt in &self.elements {
            let (raws, types) = opt.simple(parse_context, crumbs.clone())?;

            simple_raws.extend(raws);
            simple_types.extend(types);

            if !opt.is_optional() {
                // We found our first non-optional element!
                return (simple_raws, simple_types).into();
            }
        }

        // If *all* elements are optional AND simple, I guess it's also simple.
        (simple_raws, simple_types).into()
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let mut matched_segments = Vec::new();
        let mut unmatched_segments = segments.clone();
        let mut tail = Vec::new();
        let mut first_match = true;

        // Buffers of segments, not yet added.
        let mut meta_buffer = Vec::new();
        let mut non_code_buffer = Vec::new();

        for (idx, elem) in enumerate(&self.elements) {
            // 1. Handle any metas or conditionals.
            // We do this first so that it's the same whether we've run
            // out of segments or not.
            // If it's a conditional, evaluate it.
            // In both cases, we don't actually add them as inserts yet
            // because their position will depend on what types we accrue.
            if let Some(indent) = elem.as_any().downcast_ref::<Indent>() {
                meta_buffer.push(indent.clone());
                continue;
            }

            // 2. Handle any gaps in the sequence.
            // At this point we know the next element isn't a meta or conditional
            // so if we're going to look for it we need to work up to the next
            // code element (if allowed)
            if self.allow_gaps && !matched_segments.is_empty() {
                // First, if we're allowing gaps, consume any non-code.
                // NOTE: This won't consume from the end of a sequence
                // because this happens only in the run up to matching
                // another element. This is as designed. It also won't
                // happen at the *start* of a sequence either.

                for (idx, segment) in unmatched_segments.iter().enumerate() {
                    if segment.is_code() {
                        non_code_buffer.extend_from_slice(&unmatched_segments[..idx]);
                        unmatched_segments = unmatched_segments[idx..].to_vec();

                        break;
                    }
                }
            }

            // 4. Match the current element against the current position.
            let elem_match = parse_context.deeper_match(
                format!("Sequence-@{idx}"),
                false,
                &[],
                None,
                |this| elem.match_segments(unmatched_segments.clone(), this),
            )?;

            // Did we fail to match? (totally or un-cleanly)
            if !elem_match.has_match() {
                // If we can't match an element, we should ascertain whether it's
                // required. If so then fine, move on, but otherwise we should
                // crash out without a match. We have not matched the sequence.
                if elem.is_optional() {
                    // Pass this one and move onto the next element.
                    continue;
                }

                if self.parse_mode == ParseMode::Strict {
                    // In a strict mode, failing to match an element means that
                    // we don't match anything.
                    return Ok(MatchResult::from_unmatched(segments));
                }

                return Ok(MatchResult {
                    matched_segments: Vec::new(),
                    unmatched_segments: Vec::new(),
                });
            }

            // 5. Successful match: Update the buffers.
            // First flush any metas along with the gap.
            let segments = position_metas(&meta_buffer, &non_code_buffer);
            matched_segments.extend(segments);
            non_code_buffer = Vec::new();
            meta_buffer = Vec::new();

            // Add on the match itself
            matched_segments.extend(elem_match.matched_segments);
            unmatched_segments = elem_match.unmatched_segments;
            // parse_context.update_progress(matched_segments)

            if first_match && self.parse_mode == ParseMode::GreedyOnceStarted {
                // In the GREEDY_ONCE_STARTED mode, we first look ahead to find a
                // terminator after the first match (and only the first match).
                let mut terminators = parse_context.terminators.clone();
                terminators.extend(self.terminators.clone());

                (unmatched_segments, tail) = trim_to_terminator(
                    unmatched_segments.clone(),
                    tail.clone(),
                    terminators,
                    parse_context,
                )?;

                first_match = false;
            }
        }

        // TODO: After the main loop is when we would loop for terminators if
        // we are going to be greedy but only _after_ matching content.
        // Finally if we're in one of the greedy modes, and there's anything
        // left as unclaimed, mark it as unparsable.
        if matches!(self.parse_mode, ParseMode::Greedy | ParseMode::GreedyOnceStarted) {
            let (_pre, unmatched_mid, _post) = trim_non_code_segments(&unmatched_segments);
        }

        // If we finished on an optional, and so still have some unflushed metas,
        // we should do that first, then add any unmatched noncode back onto the
        // unmatched sequence.
        if !meta_buffer.is_empty() {
            matched_segments
                .extend(meta_buffer.into_iter().map(|it| it.boxed() as Box<dyn Segment>));
        }

        if !non_code_buffer.is_empty() {
            unmatched_segments = chain(non_code_buffer, unmatched_segments).collect_vec();
        }

        // If we get to here, we've matched all of the elements (or skipped them).
        // Return successfully.
        unmatched_segments.extend(tail);

        Ok(MatchResult {
            matched_segments: position_segments(&mut matched_segments, None, true),
            unmatched_segments,
        })
    }

    fn cache_key(&self) -> String {
        todo!()
    }

    fn copy(
        &self,
        insert: Option<Vec<Box<dyn Matchable>>>,
        replace_terminators: bool,
        terminators: Vec<Box<dyn Matchable>>,
    ) -> Box<dyn Matchable> {
        let mut new_elems = self.elements.clone();

        if let Some(insert) = insert {
            new_elems.extend(insert);
        }

        let mut new_grammar = self.clone();
        new_grammar.elements = new_elems;

        if replace_terminators {
            new_grammar.terminators = terminators;
        } else {
            new_grammar.terminators.extend(terminators);
        }

        new_grammar.boxed()
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Bracketed {
    bracket_type: &'static str,
    bracket_pairs_set: &'static str,
    allow_gaps: bool,

    pub this: Sequence,
}

impl Bracketed {
    pub fn new(args: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            bracket_type: "round",
            bracket_pairs_set: "bracket_pairs",
            allow_gaps: true,
            this: Sequence::new(args),
        }
    }
}

impl Bracketed {
    pub fn bracket_type(&mut self, bracket_type: &'static str) {
        self.bracket_type = bracket_type;
    }

    fn get_bracket_from_dialect(
        &self,
        parse_context: &ParseContext,
    ) -> Result<(Box<dyn Matchable>, Box<dyn Matchable>, bool), String> {
        // Assuming bracket_pairs_set and other relevant fields are part of self
        let bracket_pairs = parse_context.dialect().bracket_sets(self.bracket_pairs_set);
        for (bracket_type, start_ref, end_ref, persists) in bracket_pairs {
            if bracket_type == self.bracket_type {
                let start_bracket = parse_context.dialect().r#ref(&start_ref);
                let end_bracket = parse_context.dialect().r#ref(&end_ref);

                return Ok((start_bracket, end_bracket, persists));
            }
        }
        Err(format!(
            "bracket_type {:?} not found in bracket_pairs of {:?} dialect.",
            self.bracket_type,
            parse_context.dialect()
        ))
    }
}

impl Deref for Bracketed {
    type Target = Sequence;

    fn deref(&self) -> &Self::Target {
        &self.this
    }
}

impl DerefMut for Bracketed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.this
    }
}

impl Segment for Bracketed {}

impl Matchable for Bracketed {
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        let (start_bracket, _, _) = self.get_bracket_from_dialect(parse_context).unwrap();
        start_bracket.simple(parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        enum Status {
            Matched(MatchResult, Vec<Box<dyn Segment>>),
            EarlyReturn(MatchResult),
            Fail(SQLParseError),
        }

        // Trim ends if allowed.
        let mut seg_buff = if self.allow_gaps {
            let (_, seg_buff, _) = trim_non_code_segments(&segments);
            seg_buff.to_vec()
        } else {
            segments.clone()
        };

        // Rehydrate the bracket segments in question.
        // bracket_persists controls whether we make a BracketedSegment or not.
        let (start_bracket, end_bracket, bracket_persists) =
            self.get_bracket_from_dialect(parse_context).unwrap();

        // Allow optional override for special bracket-like things
        let start_bracket = start_bracket;
        let end_bracket = end_bracket;

        let bracket_segment;
        let content_segs;
        let trailing_segments;

        if let Some(bracketed) =
            seg_buff.first().and_then(|seg| seg.as_any().downcast_ref::<BracketedSegment>())
        {
            bracket_segment = bracketed.clone();

            if !start_bracket
                .match_segments(bracket_segment.start_bracket.clone(), parse_context)?
                .has_match()
            {
                return Ok(MatchResult::from_unmatched(segments));
            }

            let start_len = bracket_segment.start_bracket.len();
            let end_len = bracket_segment.end_bracket.len();

            content_segs = bracket_segment.segments
                [start_len..bracket_segment.segments.len() - end_len]
                .to_vec();
            trailing_segments = seg_buff[1..].to_vec();
        } else {
            // Look for the first bracket
            let status = parse_context.deeper_match("Bracketed-First", false, &[], None, |this| {
                let start_match = start_bracket.match_segments(segments.clone(), this);

                match start_match {
                    Ok(start_match) if start_match.has_match() => {
                        let unmatched_segments = start_match.unmatched_segments.clone();
                        Status::Matched(start_match, unmatched_segments)
                    }
                    Ok(_) => Status::EarlyReturn(MatchResult::from_unmatched(segments.clone())),
                    Err(err) => Status::Fail(err),
                }
            });

            let start_match = match status {
                Status::Matched(match_result, segments) => {
                    seg_buff = segments;
                    match_result
                }
                Status::EarlyReturn(match_result) => return Ok(match_result),
                Status::Fail(error) => return Err(error),
            };

            let (segs, end_match) =
                parse_context.deeper_match("Bracketed-End", true, &[], None, |this| {
                    let (content_segs, end_match, _) = bracket_sensitive_look_ahead_match(
                        seg_buff,
                        vec![end_bracket.clone()],
                        this,
                        start_bracket.into(),
                        end_bracket.into(),
                        self.bracket_pairs_set.into(),
                    )?;

                    Ok((content_segs, end_match))
                })?;

            content_segs = segs;

            if !end_match.has_match() {
                panic!("Couldn't find closing bracket for opening bracket.")
            }

            bracket_segment = BracketedSegment::new(
                chain!(
                    start_match.matched_segments.clone(),
                    content_segs.clone(),
                    end_match.matched_segments.clone()
                )
                .collect_vec(),
                start_match.matched_segments,
                end_match.matched_segments,
            );
            trailing_segments = end_match.unmatched_segments;
        }

        // Then trim whitespace and deal with the case of non-code content e.g. "(   )"
        let (pre_segs, content_segs, post_segs) = if self.allow_gaps {
            trim_non_code_segments(&content_segs)
        } else {
            (&[][..], &content_segs[..], &[][..])
        };

        // If we've got a case of empty brackets check whether that is allowed.
        if content_segs.is_empty() {
            return Ok(
                if self.this.elements.is_empty()
                    || (self.this.elements.iter().all(|e| e.is_optional())
                        && (self.allow_gaps || (pre_segs.is_empty() && post_segs.is_empty())))
                {
                    MatchResult {
                        matched_segments: bracket_segment.segments,
                        unmatched_segments: trailing_segments,
                    }
                } else {
                    MatchResult::from_unmatched(segments)
                },
            );
        }

        // Match the content using super. Sequence will interpret the content of the
        // elements. Within the brackets, clear any inherited terminators.
        let content_match = parse_context.deeper_match("Bracketed", true, &[], None, |this| {
            self.this.match_segments(content_segs.to_vec(), this)
        })?;

        // We require a complete match for the content (hopefully for obvious reasons)
        if !content_match.is_complete() {
            // No complete match. Fail.
            return Ok(MatchResult::from_unmatched(segments));
        }

        Ok(MatchResult {
            matched_segments: if bracket_persists {
                vec![bracket_segment.boxed()]
            } else {
                bracket_segment.segments
            },
            unmatched_segments: trailing_segments,
        })
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools as _;

    use super::Sequence;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::markers::PositionMarker;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::StringParser;
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::meta::Indent;
    use crate::core::parser::segments::test_functions::{
        fresh_ansi_dialect, generate_test_segments_func, test_segments,
    };
    use crate::core::parser::types::ParseMode;
    use crate::helpers::{Boxed, ToMatchable};

    #[test]
    fn test__parser__grammar_sequence() {
        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
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
                    segment.get_position_marker().unwrap().into(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        let g = Sequence::new(vec![bs.clone(), fs.clone()]);
        let gc = Sequence::new(vec![bs, fs]).allow_gaps(false);

        let match_result = g.match_segments(test_segments(), &mut ctx).unwrap();

        assert_eq!(match_result.matched_segments[0].get_raw().unwrap(), "bar");
        assert_eq!(
            match_result.matched_segments[1].get_raw().unwrap(),
            test_segments()[1].get_raw().unwrap()
        );
        assert_eq!(match_result.matched_segments[2].get_raw().unwrap(), "foo");
        assert_eq!(match_result.len(), 3);

        assert!(!gc.match_segments(test_segments(), &mut ctx).unwrap().has_match());

        assert!(!g.match_segments(test_segments()[1..].to_vec(), &mut ctx).unwrap().has_match());
    }

    #[test]
    fn test__parser__grammar_sequence_nested() {
        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
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
                    segment.get_position_marker().unwrap().into(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let bas = StringParser::new(
            "baar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let g = Sequence::new(vec![Sequence::new(vec![bs, fs]).boxed(), bas]);

        let mut ctx = ParseContext::new(fresh_ansi_dialect());

        assert!(
            !g.match_segments(test_segments()[..2].to_vec(), &mut ctx).unwrap().has_match(),
            "Expected no match, but a match was found."
        );

        let segments = g.match_segments(test_segments(), &mut ctx).unwrap().matched_segments;
        assert_eq!(segments[0].get_raw().unwrap(), "bar");
        assert_eq!(segments[1].get_raw().unwrap(), test_segments()[1].get_raw().unwrap());
        assert_eq!(segments[2].get_raw().unwrap(), "foo");
        assert_eq!(segments[3].get_raw().unwrap(), "baar");
        assert_eq!(segments.len(), 4);
    }

    #[test]
    fn test__parser__grammar_sequence_indent() {
        let bs = StringParser::new(
            "bar",
            |segment| {
                KeywordSegment::new(
                    segment.get_raw().unwrap(),
                    segment.get_position_marker().unwrap().into(),
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
                    segment.get_position_marker().unwrap().into(),
                )
                .boxed()
            },
            None,
            false,
            None,
        )
        .boxed();

        let g = Sequence::new(vec![Indent::new(PositionMarker::default()).to_matchable(), bs, fs]);
        let mut ctx = ParseContext::new(fresh_ansi_dialect());
        let segments = g.match_segments(test_segments(), &mut ctx).unwrap().matched_segments;

        assert_eq!(segments[0].get_type(), "indent");
        assert_eq!(segments[1].get_type(), "kw");
    }

    #[test]
    fn test__parser__grammar_sequence_modes() {
        let segments = generate_test_segments_func(vec!["a", " ", "b", " ", "c", "d", " ", "d"]);
        let cases: [(_, &[_], &[_], _, &[_]); 6] = [
            (ParseMode::Strict, &["a"], &[], 0..2, &[("kw", "a")]),
            (ParseMode::Strict, &["a", "b"], &[], 0..2, &[]),
            (ParseMode::Strict, &["b"], &[], 0..2, &[]),
            (ParseMode::Strict, &["a"], &[], 0..5, &[("kw", "a")]),
            (ParseMode::Strict, &["a", "c"], &[], 0..5, &[("kw", "a")]),
            (ParseMode::Strict, &["a", "x"], &["c"], 0..5, &[]),
        ];

        for (parse_mode, sequence, terminators, input_slice, output_tuple) in cases {
            let mut parse_context = ParseContext::new(fresh_ansi_dialect());

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
                            .boxed()
                        },
                        None,
                        false,
                        None,
                    )
                    .to_matchable()
                })
                .collect_vec();

            let seq = Sequence::new(elements);

            let match_result =
                seq.match_segments(segments[input_slice].to_vec(), &mut parse_context).unwrap();

            let result = match_result
                .matched_segments
                .clone()
                .into_iter()
                .map(|segment| (segment.get_type(), segment.get_raw().unwrap()))
                .collect_vec();

            let are_equal = result
                .iter()
                .map(|(s, str_ref)| (s, str_ref.as_str()))
                .zip(output_tuple.iter())
                .all(|((s1, str_ref1), (s2, str_ref2))| s1 == s2 && str_ref1 == *str_ref2);

            assert!(are_equal);
        }
    }
}
