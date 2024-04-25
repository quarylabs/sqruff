use std::iter::zip;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use ahash::AHashSet;
use itertools::{chain, enumerate, Itertools};
use uuid::Uuid;

use super::conditional::Conditional;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::helpers::{check_still_complete, trim_non_code_segments};
use crate::core::parser::match_algorithms::{
    bracket_sensitive_look_ahead_match, greedy_match, prune_options,
};
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{
    position_segments, ErasedSegment, Segment, UnparsableSegment,
};
use crate::core::parser::segments::bracketed::BracketedSegment;
use crate::core::parser::segments::meta::{Indent, MetaSegment, MetaSegmentKind};
use crate::core::parser::types::ParseMode;
use crate::helpers::ToErasedSegment;

fn trim_to_terminator(
    mut segments: Vec<ErasedSegment>,
    mut tail: Vec<ErasedSegment>,
    terminators: Vec<Rc<dyn Matchable>>,
    parse_context: &mut ParseContext,
) -> Result<(Vec<ErasedSegment>, Vec<ErasedSegment>), SQLParseError> {
    let pruned_terms = prune_options(&terminators, &segments, parse_context);

    for term in pruned_terms {
        if term.match_segments(&segments, parse_context)?.has_match() {
            return Ok((Vec::new(), chain(segments, tail).collect_vec()));
        }
    }

    let term_match =
        parse_context.deeper_match("Sequence-GreedyB-@0", false, &[], false.into(), |this| {
            greedy_match(segments.clone(), this, terminators, false)
        })?;

    if term_match.has_match() {
        // If we _do_ find a terminator, we separate off everything
        // beyond that terminator (and any preceding non-code) so that
        // it's not available to match against for the rest of this.
        tail = term_match.unmatched_segments;
        segments = term_match.matched_segments;

        for (idx, segment) in segments.iter().enumerate().rev() {
            if segment.is_code() {
                return Ok(split_and_concatenate(&segments, idx, &tail));
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
    metas: &[Indent],           // Assuming Indent is a struct or type alias
    non_code: &[ErasedSegment], // Assuming BaseSegment is a struct or type alias
) -> Vec<ErasedSegment> {
    // Assuming BaseSegment can be cloned, or you have a way to handle ownership
    // transfer

    // Check if all metas have a non-negative indent value
    if metas.iter().all(|m| m.indent_val() >= 0) {
        let mut result: Vec<ErasedSegment> = Vec::new();

        // Append metas first, then non-code elements
        for meta in metas {
            result.push(meta.clone().to_erased_segment());
        }

        for segment in non_code {
            result.push(segment.clone());
        }

        result
    } else {
        let mut result: Vec<ErasedSegment> = Vec::new();

        // Append non-code elements first, then metas
        for segment in non_code {
            result.push(segment.clone());
        }
        for meta in metas {
            result.push(meta.clone().to_erased_segment());
        }

        result
    }
}

#[derive(Debug, Clone, Hash)]
#[allow(clippy::derived_hash_with_manual_eq)]
pub struct Sequence {
    elements: Vec<Rc<dyn Matchable>>,
    parse_mode: ParseMode,
    allow_gaps: bool,
    is_optional: bool,
    terminators: Vec<Rc<dyn Matchable>>,
    cache_key: String,
}

impl Sequence {
    pub fn new(elements: Vec<Rc<dyn Matchable>>) -> Self {
        Self {
            elements,
            allow_gaps: true,
            is_optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: Uuid::new_v4().hyphenated().to_string(),
        }
    }

    pub fn optional(&mut self) {
        self.is_optional = true;
    }

    pub fn terminators(mut self, terminators: Vec<Rc<dyn Matchable>>) -> Self {
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
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        let mut simple_raws = AHashSet::new();
        let mut simple_types = AHashSet::new();

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
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let mut matched_segments = Vec::new();
        let mut unmatched_segments = segments.to_vec();
        let mut tail = Vec::new();
        let mut first_match = true;

        if self.parse_mode == ParseMode::Greedy {
            (unmatched_segments, tail) = trim_to_terminator(
                segments.to_vec(),
                tail.clone(),
                self.terminators.clone(),
                parse_context,
            )?;
        }

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
            if let Some(indent) = elem.as_any().downcast_ref::<Conditional>() {
                let match_result = indent.match_segments(segments, parse_context)?;

                let matches = match_result
                    .matched_segments
                    .into_iter()
                    .map(|it| it.as_any().downcast_ref::<Indent>().unwrap().clone());

                meta_buffer.extend(matches);

                continue;
            }

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

                let mut idx_to_split = None;
                for (idx, segment) in unmatched_segments.iter().enumerate() {
                    if segment.is_code() {
                        idx_to_split = Some(idx);
                        break;
                    }
                }

                match idx_to_split {
                    Some(idx) => {
                        non_code_buffer.extend_from_slice(&unmatched_segments[..idx]);
                        unmatched_segments = unmatched_segments.split_off(idx);
                    }
                    None => {
                        // If _all_ of it is non-code then consume all of it.
                        non_code_buffer.append(&mut unmatched_segments);
                        unmatched_segments.clear();
                    }
                }
            }

            // 3. Check we still have segments left to work on.
            // Have we prematurely run out of segments?
            if unmatched_segments.is_empty() {
                // If the current element is optional, carry on.
                if elem.is_optional() {
                    continue;
                }

                if self.parse_mode == ParseMode::Strict {
                    return Ok(MatchResult::from_unmatched(segments.to_vec()));
                }

                if matched_segments.is_empty() {
                    return Ok(MatchResult::from_unmatched(segments.to_vec()));
                }

                let matched =
                    vec![UnparsableSegment::new(matched_segments).to_erased_segment()
                        as ErasedSegment];

                return Ok(MatchResult {
                    matched_segments: matched,
                    unmatched_segments: chain(non_code_buffer, tail).collect(),
                });
            }

            // 4. Match the current element against the current position.
            let elem_match = parse_context.deeper_match(
                format!("Sequence-@{idx}"),
                false,
                &[],
                None,
                |this| elem.match_segments(&unmatched_segments, this),
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
                    return Ok(MatchResult::from_unmatched(segments.to_vec()));
                }

                if self.parse_mode == ParseMode::GreedyOnceStarted && matched_segments.is_empty() {
                    return Ok(MatchResult::from_unmatched(segments.to_vec()));
                }

                matched_segments.extend(non_code_buffer);
                matched_segments
                    .push(UnparsableSegment::new(unmatched_segments).to_erased_segment());
                return Ok(MatchResult { matched_segments, unmatched_segments: tail });
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
            let (pre, unmatched_mid, post) = trim_non_code_segments(&unmatched_segments);
            if !unmatched_mid.is_empty() {
                let unparsable_seg =
                    UnparsableSegment::new(unmatched_mid.to_vec()).to_erased_segment();

                // Here, `_position_metas` presumably modifies `matched_segments` in place or
                // returns a modified copy. Since Rust does not have tuple
                // concatenation like Python, we need to explicitly extend `matched_segments`
                // with the results of `_position_metas` and then push `unparsable_seg`.
                matched_segments
                    .extend(position_metas(&meta_buffer, &[&non_code_buffer[..], pre].concat()));
                matched_segments.push(unparsable_seg);
                meta_buffer.clear();
                non_code_buffer.clear();
                tail.extend(post.to_vec());
                unmatched_segments = vec![];
            }
        }

        // If we finished on an optional, and so still have some unflushed metas,
        // we should do that first, then add any unmatched noncode back onto the
        // unmatched sequence.
        if !meta_buffer.is_empty() {
            matched_segments
                .extend(meta_buffer.into_iter().map(|it| it.to_erased_segment() as ErasedSegment));
        }

        if !non_code_buffer.is_empty() {
            unmatched_segments = chain(non_code_buffer, unmatched_segments).collect_vec();
        }

        // If we get to here, we've matched all of the elements (or skipped them).
        // Return successfully.
        unmatched_segments.extend(tail);

        #[cfg(debug_assertions)]
        check_still_complete(segments, &matched_segments, &unmatched_segments);

        Ok(MatchResult {
            matched_segments: position_segments(&matched_segments, None, true),
            unmatched_segments,
        })
    }

    fn cache_key(&self) -> String {
        self.cache_key.clone()
    }

    fn copy(
        &self,
        insert: Option<Vec<Rc<dyn Matchable>>>,
        replace_terminators: bool,
        terminators: Vec<Rc<dyn Matchable>>,
    ) -> Rc<dyn Matchable> {
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

        Rc::new(new_grammar)
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
    pub fn new(args: Vec<Rc<dyn Matchable>>) -> Self {
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
    ) -> Result<(Rc<dyn Matchable>, Rc<dyn Matchable>, bool), String> {
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
    fn is_optional(&self) -> bool {
        self.this.is_optional()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        let (start_bracket, _, _) = self.get_bracket_from_dialect(parse_context).unwrap();
        start_bracket.simple(parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        enum Status {
            Matched(MatchResult, Vec<ErasedSegment>),
            EarlyReturn(MatchResult),
            Fail(SQLParseError),
        }

        // Trim ends if allowed.
        let mut seg_buff = if self.allow_gaps {
            let (_, seg_buff, _) = trim_non_code_segments(segments);
            seg_buff.to_vec()
        } else {
            segments.to_vec()
        };

        // Rehydrate the bracket segments in question.
        // bracket_persists controls whether we make a BracketedSegment or not.
        let (start_bracket, end_bracket, bracket_persists) =
            self.get_bracket_from_dialect(parse_context).unwrap();

        // Allow optional override for special bracket-like things
        let start_bracket = start_bracket;
        let end_bracket = end_bracket;

        let mut bracket_segment;
        let content_segs;
        let trailing_segments;

        if let Some(bracketed) =
            seg_buff.first().and_then(|seg| seg.as_any().downcast_ref::<BracketedSegment>())
        {
            bracket_segment = bracketed.clone();

            if !start_bracket
                .match_segments(&bracket_segment.start_bracket, parse_context)?
                .has_match()
            {
                return Ok(MatchResult::from_unmatched(segments.to_vec()));
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
                let start_match = start_bracket.match_segments(segments, this);

                match start_match {
                    Ok(start_match) if start_match.has_match() => {
                        let unmatched_segments = start_match.unmatched_segments.clone();
                        Status::Matched(start_match, unmatched_segments)
                    }
                    Ok(_) => Status::EarlyReturn(MatchResult::from_unmatched(segments.to_vec())),
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
            let segment = if self.elements.is_empty()
                || (self.elements.iter().all(|e| e.is_optional())
                    && (self.allow_gaps || (pre_segs.is_empty() && post_segs.is_empty())))
            {
                MatchResult {
                    matched_segments: if bracket_persists {
                        vec![bracket_segment.to_erased_segment()]
                    } else {
                        bracket_segment.segments
                    },
                    unmatched_segments: trailing_segments,
                }
            } else {
                MatchResult::from_unmatched(segments.to_vec())
            };

            return Ok(segment);
        }

        // Match the content using super. Sequence will interpret the content of the
        // elements. Within the brackets, clear any inherited terminators.
        let content_match = parse_context.deeper_match("Bracketed", true, &[], None, |this| {
            self.this.match_segments(content_segs, this)
        })?;

        // We require a complete match for the content (hopefully for obvious reasons)
        if !content_match.is_complete() {
            // No complete match. Fail.
            return Ok(MatchResult::from_unmatched(segments.to_vec()));
        }

        bracket_segment.segments = chain!(
            bracket_segment.start_bracket.clone(),
            Some(MetaSegment::indent().to_erased_segment()),
            pre_segs.iter().cloned(),
            content_match.all_segments(),
            post_segs.iter().cloned(),
            Some(MetaSegment::dedent().to_erased_segment()),
            bracket_segment.end_bracket.clone()
        )
        .collect_vec();

        Ok(MatchResult {
            matched_segments: if bracket_persists {
                vec![bracket_segment.to_erased_segment()]
            } else {
                bracket_segment.segments
            },
            unmatched_segments: trailing_segments,
        })
    }

    fn cache_key(&self) -> String {
        self.this.cache_key()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use itertools::Itertools;
    use serde_json::{json, Value};

    use super::Sequence;
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::parsers::StringParser;
    use crate::core::parser::segments::keyword::KeywordSegment;
    use crate::core::parser::segments::meta::MetaSegment;
    use crate::core::parser::segments::test_functions::{
        fresh_ansi_dialect, generate_test_segments_func, test_segments,
    };
    use crate::core::parser::types::ParseMode;
    use crate::helpers::{ToErasedSegment, ToMatchable};

    #[test]
    fn test__parser__grammar_sequence() {
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

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        let g = Sequence::new(vec![bs.clone(), fs.clone()]);
        let gc = Sequence::new(vec![bs, fs]).allow_gaps(false);

        let match_result = g.match_segments(&test_segments(), &mut ctx).unwrap();

        assert_eq!(match_result.matched_segments[0].get_raw().unwrap(), "bar");
        assert_eq!(
            match_result.matched_segments[1].get_raw().unwrap(),
            test_segments()[1].get_raw().unwrap()
        );
        assert_eq!(match_result.matched_segments[2].get_raw().unwrap(), "foo");
        assert_eq!(match_result.len(), 3);

        assert!(!gc.match_segments(&test_segments(), &mut ctx).unwrap().has_match());

        assert!(!g.match_segments(&test_segments()[1..], &mut ctx).unwrap().has_match());
    }

    #[test]
    fn test__parser__grammar_sequence_nested() {
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
        )) as Rc<dyn Matchable>;

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
        )) as Rc<dyn Matchable>;

        let bas = Rc::new(StringParser::new(
            "baar",
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
        )) as Rc<dyn Matchable>;

        let g = Sequence::new(vec![Rc::new(Sequence::new(vec![bs, fs])), bas]);

        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        assert!(
            !g.match_segments(&test_segments()[..2], &mut ctx).unwrap().has_match(),
            "Expected no match, but a match was found."
        );

        let segments = g.match_segments(&test_segments(), &mut ctx).unwrap().matched_segments;
        assert_eq!(segments[0].get_raw().unwrap(), "bar");
        assert_eq!(segments[1].get_raw().unwrap(), test_segments()[1].get_raw().unwrap());
        assert_eq!(segments[2].get_raw().unwrap(), "foo");
        assert_eq!(segments[3].get_raw().unwrap(), "baar");
        assert_eq!(segments.len(), 4);
    }

    #[test]
    fn test__parser__grammar_sequence_indent() {
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

        let g = Sequence::new(vec![Rc::new(MetaSegment::indent()), bs, fs]);
        let dialect = fresh_ansi_dialect();
        let mut ctx = ParseContext::new(&dialect, <_>::default());
        let segments = g.match_segments(&test_segments(), &mut ctx).unwrap().matched_segments;

        assert_eq!(segments[0].get_type(), "indent");
        assert_eq!(segments[1].get_type(), "keyword");
    }

    #[test]
    fn test__parser__grammar_sequence_modes() {
        let segments = generate_test_segments_func(vec!["a", " ", "b", " ", "c", "d", " ", "d"]);
        let cases: &[(_, &[&str], &[&str], _, Value)] = &[
            // #####
            // Test matches where we should get something, and that's
            // the whole sequence.
            // NOTE: Include a little whitespace in the slice (i.e. the first _two_
            (ParseMode::Strict, &["a"], &[], 0..2, json!([{"keyword": "a"}])),
            (ParseMode::Greedy, &["a"], &[], 0..2, json!([{"keyword": "a"}])),
            (ParseMode::GreedyOnceStarted, &["a"], &[], 0..2, json!([{"keyword": "a"}])),
            // #####
            // Test matching on sequences where we run out of segments before matching
            // the whole sequence.
            // STRICT returns no match.
            (ParseMode::Strict, &["a", "b"], &[], 0..2, json!([])),
            // GREEDY & GREEDY_ONCE_STARTED returns the content as unparsable, and
            // still don't include the trailing whitespace. The return value does
            // however have the matched "a" as a keyword and not a raw.
            (
                ParseMode::Greedy,
                &["a", "b"],
                &[],
                0..2,
                json!([{
                    "unparsable": [{"keyword": "a"}]
                }]),
            ),
            (
                ParseMode::GreedyOnceStarted,
                &["a", "b"],
                &[],
                0..2,
                json!([{
                    "unparsable": [{"keyword": "a"}]
                }]),
            ),
            // #####
            // Test matching on sequences where we fail to match the first element.
            // STRICT & GREEDY_ONCE_STARTED return no match.
            (ParseMode::Strict, &["b"], &[], 0..2, json!([])),
            (ParseMode::GreedyOnceStarted, &["b"], &[], 0..2, json!([])),
            // GREEDY claims the remaining elements (unmutated) as unparsable, but
            // does not claim any trailing whitespace.
            (ParseMode::Greedy, &["b"], &[], 0..2, json!([{"unparsable": [{"": "a"}]}])),
            // #####
            // Test matches where we should match the sequence fully, but there's more
            // to match.
            // First without terminators...
            // STRICT ignores the rest.
            (ParseMode::Strict, &["a"], &[], 0..5, json!([{"keyword": "a"}])),
            // The GREEDY modes claim the rest as unparsable.
            // NOTE: the whitespace in between is _not_ unparsable.
            (
                ParseMode::Greedy,
                &["a"],
                &[],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"},
                            {"whitespace": " "},
                            {"": "c"}
                        ]
                    }
                ]),
            ),
            (
                ParseMode::GreedyOnceStarted,
                &["a"],
                &[],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"},
                            {"whitespace": " "},
                            {"": "c"}
                        ]
                    }
                ]),
            ),
            // Second *with* terminators.
            // NOTE: The whitespace before the terminator is not included.
            (ParseMode::Strict, &["a"], &["c"], 0..5, json!([{"keyword": "a"}])),
            (
                ParseMode::Greedy,
                &["a"],
                &["c"],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"}
                        ]
                    }
                ]),
            ),
            (
                ParseMode::GreedyOnceStarted,
                &["a"],
                &["c"],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"}
                        ]
                    }
                ]),
            ),
            // #####
            // Test matches where we match the first element of a sequence but not the
            // second (with terminators)
            (ParseMode::Strict, &["a", "x"], &["c"], 0..5, json!([])),
            // NOTE: For GREEDY modes, the matched portion is not included as an "unparsable"
            // only the portion which failed to match. The terminator is not included and
            // the matched portion is still mutated correctly.
            (
                ParseMode::Greedy,
                &["a", "x"],
                &["c"],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"}
                        ]
                    }
                ]),
            ),
            (
                ParseMode::GreedyOnceStarted,
                &["a", "x"],
                &["c"],
                0..5,
                json!([
                    {"keyword": "a"},
                    {"whitespace": " "},
                    {
                        "unparsable": [
                            {"": "b"}
                        ]
                    }
                ]),
            ),
            // #####
            // Test competition between sequence elements and terminators.
            // In GREEDY_ONCE_STARTED, the first element is matched before any terminators.
            (ParseMode::GreedyOnceStarted, &["a"], &["a"], 0..2, json!([{"keyword": "a"}])),
            // In GREEDY, the terminator is matched first and so takes precedence.
            (ParseMode::Greedy, &["a"], &["a"], 0..2, json!([])),
            // NOTE: In these last two cases, the "b" isn't included because it acted as
            // a terminator before being considered in the sequence.
            (
                ParseMode::GreedyOnceStarted,
                &["a", "b"],
                &["b"],
                0..3,
                json!([{"unparsable": [{"keyword": "a"}]}]),
            ),
            (
                ParseMode::Greedy,
                &["a", "b"],
                &["b"],
                0..3,
                json!([{"unparsable": [{"keyword": "a"}]}]),
            ),
        ];

        for (parse_mode, sequence, terminators, input_slice, output) in cases {
            let dialect = fresh_ansi_dialect();
            let mut parse_context = ParseContext::new(&dialect, <_>::default());

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

            let mut seq = Sequence::new(elements);
            seq.terminators = terminators
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
                .collect();
            seq.parse_mode = *parse_mode;

            let match_result =
                seq.match_segments(&segments[input_slice.clone()], &mut parse_context).unwrap();

            let result = match_result
                .matched_segments
                .iter()
                .map(|it| it.to_serialised(false, true, false))
                .collect_vec();

            let input = serde_json::to_value(result).unwrap();
            assert_eq!(&input, output);
        }
    }
}
