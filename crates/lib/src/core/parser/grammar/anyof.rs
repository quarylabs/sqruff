use std::sync::Arc;

use ahash::AHashSet;
use itertools::{chain, Itertools};
use nohash_hasher::IntMap;

use super::sequence::{Bracketed, Sequence};
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_algorithms::{
    longest_match, skip_start_index_forward_to_code, trim_to_terminator,
};
use crate::core::parser::match_result::{MatchResult, Matched, Span};
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment};
use crate::core::parser::types::ParseMode;
use crate::dialects::SyntaxKind;
use crate::helpers::{next_cache_key, ToMatchable};

fn parse_mode_match_result(
    segments: &[ErasedSegment],
    current_match: MatchResult,
    max_idx: u32,
    parse_mode: ParseMode,
) -> MatchResult {
    if parse_mode == ParseMode::Strict {
        return current_match;
    }

    let stop_idx = current_match.span.end;
    if stop_idx == max_idx
        || segments[stop_idx as usize..max_idx as usize].iter().all(|it| !it.is_code())
    {
        return current_match;
    }

    let trim_idx = skip_start_index_forward_to_code(segments, stop_idx, segments.len() as u32);

    let unmatched_match = MatchResult {
        span: Span { start: trim_idx, end: max_idx },
        matched: Matched::SyntaxKind(SyntaxKind::Unparsable).into(),
        ..MatchResult::default()
    };

    current_match.append(unmatched_match)
}

pub fn simple(
    elements: &[Arc<dyn Matchable>],
    parse_context: &ParseContext,
    crumbs: Option<Vec<&str>>,
) -> Option<(AHashSet<String>, AHashSet<&'static str>)> {
    let option_simples: Vec<Option<(AHashSet<String>, AHashSet<&'static str>)>> =
        elements.iter().map(|opt| opt.simple(parse_context, crumbs.clone())).collect();

    if option_simples.iter().any(Option::is_none) {
        return None;
    }

    let simple_buff: Vec<(AHashSet<String>, AHashSet<&'static str>)> =
        option_simples.into_iter().flatten().collect();

    let simple_raws: AHashSet<_> = simple_buff.iter().flat_map(|(raws, _)| raws).cloned().collect();

    let simple_types: AHashSet<&'static str> =
        simple_buff.iter().flat_map(|(_, types)| types).cloned().collect();

    Some((simple_raws, simple_types))
}

#[derive(Debug, Clone)]
pub struct AnyNumberOf {
    pub(crate) exclude: Option<Arc<dyn Matchable>>,
    pub(crate) elements: Vec<Arc<dyn Matchable>>,
    pub(crate) terminators: Vec<Arc<dyn Matchable>>,
    pub(crate) reset_terminators: bool,
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
            reset_terminators: false,
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
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if let Some(exclude) = &self.exclude {
            let match_result = parse_context
                .deeper_match(false, &[], |ctx| exclude.match_segments(segments, idx, ctx))?;

            if match_result.has_match() {
                return Ok(MatchResult::empty_at(idx));
            }
        }

        let mut n_matches = 0;
        let mut option_counter: IntMap<_, usize> =
            self.elements.iter().map(|elem| (elem.cache_key(), 0)).collect();
        let mut matched_idx = idx;
        let mut working_idx = idx;
        let mut matched = MatchResult::empty_at(idx);
        let mut max_idx = segments.len() as u32;

        if self.parse_mode == ParseMode::Greedy {
            let terminators = if self.reset_terminators {
                self.terminators.clone()
            } else {
                chain(self.terminators.clone(), parse_context.terminators.clone()).collect_vec()
            };
            max_idx = trim_to_terminator(segments, idx, &terminators, parse_context)?;
        }

        loop {
            if (n_matches >= self.min_times && matched_idx >= max_idx)
                || self.max_times.is_some() && Some(n_matches) >= self.max_times
            {
                return Ok(parse_mode_match_result(segments, matched, max_idx, self.parse_mode));
            }

            if matched_idx >= max_idx {
                return Ok(MatchResult::empty_at(idx));
            }

            let (match_result, matched_option) =
                parse_context.deeper_match(self.reset_terminators, &self.terminators, |ctx| {
                    longest_match(&segments[..max_idx as usize], &self.elements, working_idx, ctx)
                })?;

            if !match_result.has_match() {
                if n_matches < self.min_times {
                    matched = MatchResult::empty_at(idx);
                }

                return Ok(parse_mode_match_result(segments, matched, max_idx, self.parse_mode));
            }

            let matched_option = matched_option.unwrap();
            let matched_key = matched_option.cache_key();

            if let Some(counter) = option_counter.get_mut(&matched_key) {
                *counter += 1;

                if let Some(max_times_per_element) = self.max_times_per_element
                    && *counter > max_times_per_element
                {
                    return Ok(parse_mode_match_result(
                        segments,
                        matched,
                        max_idx,
                        self.parse_mode,
                    ));
                }
            }

            matched = matched.append(match_result);
            matched_idx = matched.span.end;
            working_idx = matched_idx;
            if self.allow_gaps {
                working_idx =
                    skip_start_index_forward_to_code(segments, matched_idx, segments.len() as u32);
            }
            n_matches += 1;
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
    any_number_of.max_times = None;
    any_number_of.max_times_per_element = Some(1);
    any_number_of
}
