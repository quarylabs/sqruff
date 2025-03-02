use std::iter::zip;
use std::ops::{Deref, DerefMut};

use ahash::AHashSet;

use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;
use crate::helpers::ToMatchable;
use crate::parser::context::ParseContext;
use crate::parser::match_algorithms::{
    resolve_bracket, skip_start_index_forward_to_code, skip_stop_index_backward_to_code,
    trim_to_terminator,
};
use crate::parser::match_result::{MatchResult, Matched, Span};
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};
use crate::parser::segments::base::ErasedSegment;
use crate::parser::types::ParseMode;

fn flush_metas(
    tpre_nc_idx: u32,
    post_nc_idx: u32,
    meta_buffer: Vec<SyntaxKind>,
    _segments: &[ErasedSegment],
) -> Vec<(u32, SyntaxKind)> {
    let meta_idx = if meta_buffer.iter().all(|it| it.indent_val() >= 0) {
        tpre_nc_idx
    } else {
        post_nc_idx
    };
    meta_buffer.into_iter().map(|it| (meta_idx, it)).collect()
}

#[derive(Debug, Clone)]
pub struct Sequence {
    elements: Vec<Matchable>,
    pub parse_mode: ParseMode,
    pub allow_gaps: bool,
    is_optional: bool,
    pub terminators: Vec<Matchable>,
    cache_key: MatchableCacheKey,
}

impl Sequence {
    pub fn disallow_gaps(&mut self) {
        self.allow_gaps = false;
    }
}

impl Sequence {
    pub fn new(elements: Vec<Matchable>) -> Self {
        Self {
            elements,
            allow_gaps: true,
            is_optional: false,
            parse_mode: ParseMode::Strict,
            terminators: Vec::new(),
            cache_key: next_matchable_cache_key(),
        }
    }

    pub fn optional(&mut self) {
        self.is_optional = true;
    }

    pub fn terminators(mut self, terminators: Vec<Matchable>) -> Self {
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
        zip(&self.elements, &other.elements).all(|(a, b)| a == b)
    }
}

impl MatchableTrait for Sequence {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.is_optional
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let mut simple_raws = AHashSet::new();
        let mut simple_types = SyntaxSet::EMPTY;

        for opt in &self.elements {
            let (raws, types) = opt.simple(parse_context, crumbs.clone())?;

            simple_raws.extend(raws);
            simple_types.extend(types);

            if !opt.is_optional() {
                return Some((simple_raws, simple_types));
            }
        }

        (simple_raws, simple_types).into()
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        mut idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let start_idx = idx;
        let mut matched_idx = idx;
        let mut max_idx = segments.len() as u32;
        let mut insert_segments = Vec::new();
        let mut child_matches = Vec::new();
        let mut first_match = true;
        let mut meta_buffer = Vec::new();

        if self.parse_mode == ParseMode::Greedy {
            let terminators =
                [self.terminators.clone(), parse_context.terminators.clone()].concat();

            max_idx = trim_to_terminator(segments, idx, &terminators, parse_context)?;
        }

        for elem in &self.elements {
            if let Some(indent) = elem.as_conditional() {
                let match_result = indent.match_segments(segments, matched_idx, parse_context)?;
                for (_, submatch) in match_result.insert_segments {
                    meta_buffer.push(submatch);
                }
                continue;
            } else if let Some(indent) = elem.as_indent() {
                meta_buffer.push(indent.kind);
                continue;
            }

            idx = if self.allow_gaps {
                skip_start_index_forward_to_code(segments, matched_idx, max_idx)
            } else {
                matched_idx
            };

            if idx >= max_idx {
                if elem.is_optional() {
                    continue;
                }

                if self.parse_mode == ParseMode::Strict || matched_idx == start_idx {
                    return Ok(MatchResult::empty_at(idx));
                }

                insert_segments.extend(meta_buffer.into_iter().map(|meta| (matched_idx, meta)));

                return Ok(MatchResult {
                    span: Span {
                        start: start_idx,
                        end: matched_idx,
                    },
                    insert_segments,
                    child_matches,
                    matched: Matched::SyntaxKind(SyntaxKind::Unparsable).into(),
                });
            }

            let mut elem_match = parse_context.deeper_match(false, &[], |ctx| {
                elem.match_segments(&segments[..max_idx as usize], idx, ctx)
            })?;

            if !elem_match.has_match() {
                if elem.is_optional() {
                    continue;
                }

                if self.parse_mode == ParseMode::Strict {
                    return Ok(MatchResult::empty_at(idx));
                }

                if self.parse_mode == ParseMode::GreedyOnceStarted && matched_idx == start_idx {
                    return Ok(MatchResult::empty_at(idx));
                }

                if matched_idx == start_idx {
                    return Ok(MatchResult {
                        span: Span {
                            start: start_idx,
                            end: max_idx,
                        },
                        matched: Matched::SyntaxKind(SyntaxKind::Unparsable).into(),
                        ..MatchResult::default()
                    });
                }

                child_matches.push(MatchResult {
                    span: Span {
                        start: skip_start_index_forward_to_code(segments, matched_idx, max_idx),
                        end: max_idx,
                    },
                    matched: Matched::SyntaxKind(SyntaxKind::Unparsable).into(),
                    ..MatchResult::default()
                });

                return Ok(MatchResult {
                    span: Span {
                        start: start_idx,
                        end: max_idx,
                    },
                    insert_segments,
                    child_matches,
                    matched: None,
                });
            }

            let meta_buffer = std::mem::take(&mut meta_buffer);
            insert_segments.append(&mut flush_metas(matched_idx, idx, meta_buffer, segments));

            matched_idx = elem_match.span.end;

            if first_match && self.parse_mode == ParseMode::GreedyOnceStarted {
                let terminators =
                    [self.terminators.clone(), parse_context.terminators.clone()].concat();
                max_idx = trim_to_terminator(segments, matched_idx, &terminators, parse_context)?;
                first_match = false;
            }

            if elem_match.matched.is_some() {
                child_matches.push(elem_match);
                continue;
            }

            child_matches.append(&mut elem_match.child_matches);
            insert_segments.append(&mut elem_match.insert_segments);
        }

        insert_segments.extend(meta_buffer.into_iter().map(|meta| (matched_idx, meta)));

        if matches!(
            self.parse_mode,
            ParseMode::Greedy | ParseMode::GreedyOnceStarted
        ) && max_idx > matched_idx
        {
            let idx = skip_start_index_forward_to_code(segments, matched_idx, max_idx);
            let stop_idx = skip_stop_index_backward_to_code(segments, max_idx, idx);

            if stop_idx > idx {
                child_matches.push(MatchResult {
                    span: Span {
                        start: idx,
                        end: stop_idx,
                    },
                    matched: Matched::SyntaxKind(SyntaxKind::Unparsable).into(),
                    ..Default::default()
                });
                matched_idx = stop_idx;
            }
        }

        Ok(MatchResult {
            span: Span {
                start: start_idx,
                end: matched_idx,
            },
            matched: None,
            insert_segments,
            child_matches,
        })
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }

    fn copy(
        &self,
        insert: Option<Vec<Matchable>>,
        at: Option<usize>,
        before: Option<Matchable>,
        remove: Option<Vec<Matchable>>,
        terminators: Vec<Matchable>,
        replace_terminators: bool,
    ) -> Matchable {
        let mut new_elements = self.elements.clone();

        if let Some(insert_elements) = insert {
            if let Some(before_element) = before {
                if let Some(index) = self.elements.iter().position(|e| e == &before_element) {
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
            new_elements.retain(|elem| !remove_elements.contains(elem));
        }

        let mut new_grammar = self.clone();

        new_grammar.elements = new_elements;
        new_grammar.terminators = if replace_terminators {
            terminators
        } else {
            [self.terminators.clone(), terminators].concat()
        };

        new_grammar.to_matchable()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bracketed {
    pub bracket_type: &'static str,
    pub bracket_pairs_set: &'static str,
    allow_gaps: bool,
    pub this: Sequence,
}

impl Bracketed {
    pub fn new(args: Vec<Matchable>) -> Self {
        Self {
            bracket_type: "round",
            bracket_pairs_set: "bracket_pairs",
            allow_gaps: true,
            this: Sequence::new(args),
        }
    }
}

type BracketInfo = Result<(Matchable, Matchable, bool), String>;

impl Bracketed {
    pub fn bracket_type(&mut self, bracket_type: &'static str) {
        self.bracket_type = bracket_type;
    }

    fn get_bracket_from_dialect(&self, parse_context: &ParseContext) -> BracketInfo {
        let bracket_pairs = parse_context.dialect().bracket_sets(self.bracket_pairs_set);
        for (bracket_type, start_ref, end_ref, persists) in bracket_pairs {
            if bracket_type == self.bracket_type {
                let start_bracket = parse_context.dialect().r#ref(start_ref);
                let end_bracket = parse_context.dialect().r#ref(end_ref);

                return Ok((start_bracket, end_bracket, persists));
            }
        }
        Err(format!(
            "bracket_type {:?} not found in bracket_pairs ({}) of {:?} dialect.",
            self.bracket_type,
            self.bracket_pairs_set,
            parse_context.dialect().name
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

impl MatchableTrait for Bracketed {
    fn elements(&self) -> &[Matchable] {
        &self.elements
    }

    fn is_optional(&self) -> bool {
        self.this.is_optional()
    }

    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        let (start_bracket, _, _) = self.get_bracket_from_dialect(parse_context).unwrap();
        start_bracket.simple(parse_context, crumbs)
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        let (start_bracket, end_bracket, bracket_persists) =
            self.get_bracket_from_dialect(parse_context).unwrap();

        let start_match = parse_context.deeper_match(false, &[], |ctx| {
            start_bracket.match_segments(segments, idx, ctx)
        })?;

        if !start_match.has_match() {
            return Ok(MatchResult::empty_at(idx));
        }

        let start_match_span = start_match.span;

        let bracketed_match = resolve_bracket(
            segments,
            start_match,
            start_bracket.clone(),
            &[start_bracket],
            &[end_bracket.clone()],
            &[bracket_persists],
            parse_context,
            false,
        )?;

        let mut idx = start_match_span.end;
        let mut end_idx = bracketed_match.span.end - 1;

        if self.allow_gaps {
            idx = skip_start_index_forward_to_code(segments, idx, segments.len() as u32);
            end_idx = skip_stop_index_backward_to_code(segments, end_idx, idx);
        }

        let mut content_match =
            parse_context.deeper_match(true, &[end_bracket.clone()], |ctx| {
                self.this
                    .match_segments(&segments[..end_idx as usize], idx, ctx)
            })?;

        if content_match.span.end != end_idx && self.parse_mode == ParseMode::Strict {
            return Ok(MatchResult::empty_at(idx));
        }

        let intermediate_slice = Span {
            start: content_match.span.end,
            end: bracketed_match.span.end - 1,
        };

        if !self.allow_gaps && intermediate_slice.start == intermediate_slice.end {
            unimplemented!()
        }

        let mut child_matches = bracketed_match.child_matches;
        if content_match.matched.is_some() {
            child_matches.push(content_match);
        } else {
            child_matches.append(&mut content_match.child_matches);
        }

        Ok(MatchResult {
            child_matches,
            ..bracketed_match
        })
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.this.cache_key()
    }
}
