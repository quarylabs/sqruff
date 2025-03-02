use ahash::AHashMap;
use itertools::{Itertools as _, enumerate, multiunzip};
use smol_str::StrExt;

use super::context::ParseContext;
use super::match_result::{MatchResult, Matched, Span};
use super::matchable::{Matchable, MatchableTrait};
use super::segments::base::ErasedSegment;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;

pub fn skip_start_index_forward_to_code(
    segments: &[ErasedSegment],
    start_idx: u32,
    max_idx: u32,
) -> u32 {
    let mut idx = start_idx;
    while idx < max_idx {
        if segments[idx as usize].is_code() {
            break;
        }
        idx += 1;
    }
    idx
}

pub fn skip_stop_index_backward_to_code(
    segments: &[ErasedSegment],
    stop_idx: u32,
    min_idx: u32,
) -> u32 {
    let mut idx = stop_idx;
    while idx > min_idx {
        if segments[idx as usize - 1].is_code() {
            break;
        }
        idx -= 1;
    }
    idx
}

pub fn first_trimmed_raw(seg: &ErasedSegment) -> String {
    seg.raw()
        .to_uppercase_smolstr()
        .split(char::is_whitespace)
        .next()
        .map(ToString::to_string)
        .unwrap_or_default()
}

pub fn first_non_whitespace(
    segments: &[ErasedSegment],
    start_idx: u32,
) -> Option<(String, &SyntaxSet)> {
    for segment in segments.iter().skip(start_idx as usize) {
        if let Some(raw) = segment.first_non_whitespace_segment_raw_upper() {
            return Some((raw, segment.class_types()));
        }
    }

    None
}

pub fn prune_options(
    options: &[Matchable],
    segments: &[ErasedSegment],
    parse_context: &mut ParseContext,
    start_idx: u32,
) -> Vec<Matchable> {
    let mut available_options = vec![];

    // Find the first code element to match against.
    let Some((first_raw, first_types)) = first_non_whitespace(segments, start_idx) else {
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

        if !matched && first_types.intersects(&simple_types) {
            available_options.push(opt.clone());
        }
    }

    available_options
}

pub fn longest_match(
    segments: &[ErasedSegment],
    matchers: &[Matchable],
    idx: u32,
    parse_context: &mut ParseContext,
) -> Result<(MatchResult, Option<Matchable>), SQLParseError> {
    let max_idx = segments.len() as u32;

    if matchers.is_empty() || idx == max_idx {
        return Ok((MatchResult::empty_at(idx), None));
    }

    let available_options = prune_options(matchers, segments, parse_context, idx);
    let available_options_count = available_options.len();

    if available_options.is_empty() {
        return Ok((MatchResult::empty_at(idx), None));
    }

    let terminators = parse_context.terminators.clone();
    let cache_position = segments[idx as usize].get_position_marker().unwrap();

    let loc_key = (
        segments[idx as usize].raw().clone(),
        cache_position.working_loc(),
        segments[idx as usize].get_type(),
        max_idx,
    );

    let loc_key = parse_context.loc_key(loc_key);

    let mut best_match = MatchResult::empty_at(idx);
    let mut best_matcher = None;

    'matcher: for (matcher_idx, matcher) in enumerate(available_options) {
        let matcher_key = matcher.cache_key();
        let res_match = parse_context.check_parse_cache(loc_key, matcher_key);

        let res_match = match res_match {
            Some(res_match) => res_match,
            None => {
                let res_match = matcher.match_segments(segments, idx, parse_context)?;
                parse_context.put_parse_cache(loc_key, matcher_key, res_match.clone());
                res_match
            }
        };

        if res_match.has_match() && res_match.span.end == max_idx {
            return Ok((res_match, matcher.into()));
        }

        if res_match.is_better_than(&best_match) {
            best_match = res_match;
            best_matcher = matcher.into();

            if matcher_idx == available_options_count - 1 {
                break 'matcher;
            } else if !terminators.is_empty() {
                let next_code_idx = skip_start_index_forward_to_code(
                    segments,
                    best_match.span.end,
                    segments.len() as u32,
                );

                if next_code_idx == segments.len() as u32 {
                    break 'matcher;
                }

                for terminator in &terminators {
                    let terminator_match =
                        terminator.match_segments(segments, next_code_idx, parse_context)?;

                    if terminator_match.has_match() {
                        break 'matcher;
                    }
                }
            }
        }
    }

    Ok((best_match, best_matcher))
}

fn next_match(
    segments: &[ErasedSegment],
    idx: u32,
    matchers: &[Matchable],
    parse_context: &mut ParseContext,
) -> Result<(MatchResult, Option<Matchable>), SQLParseError> {
    let max_idx = segments.len() as u32;

    if idx >= max_idx {
        return Ok((MatchResult::empty_at(idx), None));
    }

    let mut raw_simple_map: AHashMap<String, Vec<usize>> = AHashMap::new();
    let mut type_simple_map: AHashMap<SyntaxKind, Vec<usize>> = AHashMap::new();

    for (idx, matcher) in enumerate(matchers) {
        let (raws, types) = matcher.simple(parse_context, None).unwrap();

        raw_simple_map.reserve(raws.len());
        type_simple_map.reserve(types.len());

        for raw in raws {
            raw_simple_map.entry(raw).or_default().push(idx);
        }

        for typ in types {
            type_simple_map.entry(typ).or_default().push(idx);
        }
    }

    for idx in idx..max_idx {
        let seg = &segments[idx as usize];
        let mut matcher_idxs = raw_simple_map
            .get(&first_trimmed_raw(seg))
            .cloned()
            .unwrap_or_default();

        let keys = type_simple_map.keys().copied().collect();
        let type_overlap = seg.class_types().clone().intersection(&keys);

        for typ in type_overlap {
            matcher_idxs.extend(type_simple_map[&typ].clone());
        }

        if matcher_idxs.is_empty() {
            continue;
        }

        matcher_idxs.sort();
        for matcher_idx in matcher_idxs {
            let matcher = &matchers[matcher_idx];
            let match_result = matcher.match_segments(segments, idx, parse_context)?;

            if match_result.has_match() {
                return Ok((match_result, matcher.clone().into()));
            }
        }
    }

    Ok((MatchResult::empty_at(idx), None))
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_bracket(
    segments: &[ErasedSegment],
    opening_match: MatchResult,
    opening_matcher: Matchable,
    start_brackets: &[Matchable],
    end_brackets: &[Matchable],
    bracket_persists: &[bool],
    parse_context: &mut ParseContext,
    nested_match: bool,
) -> Result<MatchResult, SQLParseError> {
    let type_idx = start_brackets
        .iter()
        .position(|it| it == &opening_matcher)
        .unwrap();
    let mut matched_idx = opening_match.span.end;
    let mut child_matches = vec![opening_match.clone()];

    let matchers = [start_brackets, end_brackets].concat();
    loop {
        let (match_result, matcher) = next_match(segments, matched_idx, &matchers, parse_context)?;

        if !match_result.has_match() {
            return Err(SQLParseError {
                description: "Couldn't find closing bracket for opening bracket.".into(),
                segment: segments[opening_match.span.start as usize].clone().into(),
            });
        }

        let matcher = matcher.unwrap();
        if end_brackets.contains(&matcher) {
            let closing_idx = end_brackets.iter().position(|it| it == &matcher).unwrap();

            if closing_idx == type_idx {
                let match_span = match_result.span;
                let persists = bracket_persists[type_idx];
                let insert_segments = vec![
                    (opening_match.span.end, SyntaxKind::Indent),
                    (match_result.span.start, SyntaxKind::Dedent),
                ];

                child_matches.push(match_result);
                let match_result = MatchResult {
                    span: Span {
                        start: opening_match.span.start,
                        end: match_span.end,
                    },
                    matched: None,
                    insert_segments,
                    child_matches,
                };

                if !persists {
                    return Ok(match_result);
                }

                return Ok(match_result.wrap(Matched::SyntaxKind(SyntaxKind::Bracketed)));
            }

            return Err(SQLParseError {
                description: "Found unexpected end bracket!".into(),
                segment: segments[(match_result.span.end - 1) as usize]
                    .clone()
                    .into(),
            });
        }

        let inner_match = resolve_bracket(
            segments,
            match_result,
            matcher,
            start_brackets,
            end_brackets,
            bracket_persists,
            parse_context,
            false,
        )?;

        matched_idx = inner_match.span.end;
        if nested_match {
            child_matches.push(inner_match);
        }
    }
}

type BracketMatch = Result<(MatchResult, Option<Matchable>, Vec<MatchResult>), SQLParseError>;

fn next_ex_bracket_match(
    segments: &[ErasedSegment],
    idx: u32,
    matchers: &[Matchable],
    parse_context: &mut ParseContext,
    bracket_pairs_set: &'static str,
) -> BracketMatch {
    let max_idx = segments.len() as u32;

    if idx >= max_idx {
        return Ok((MatchResult::empty_at(idx), None, Vec::new()));
    }

    let (_, start_bracket_refs, end_bracket_refs, bracket_persists): (
        Vec<_>,
        Vec<_>,
        Vec<_>,
        Vec<_>,
    ) = multiunzip(parse_context.dialect().bracket_sets(bracket_pairs_set));

    let start_brackets = start_bracket_refs
        .into_iter()
        .map(|seg_ref| parse_context.dialect().r#ref(seg_ref))
        .collect_vec();

    let end_brackets = end_bracket_refs
        .into_iter()
        .map(|seg_ref| parse_context.dialect().r#ref(seg_ref))
        .collect_vec();

    let all_matchers = [matchers, &start_brackets, &end_brackets].concat();

    let mut matched_idx = idx;
    let mut child_matches: Vec<MatchResult> = Vec::new();

    loop {
        let (match_result, matcher) =
            next_match(segments, matched_idx, &all_matchers, parse_context)?;
        if !match_result.has_match() {
            return Ok((match_result, matcher.clone(), child_matches));
        }

        if let Some(matcher) = matcher
            .as_ref()
            .filter(|matcher| matchers.contains(matcher))
        {
            return Ok((match_result, Some(matcher.clone()), child_matches));
        }

        if matcher
            .as_ref()
            .is_some_and(|matcher| end_brackets.contains(matcher))
        {
            return Ok((MatchResult::empty_at(idx), None, Vec::new()));
        }

        let bracket_match = resolve_bracket(
            segments,
            match_result,
            matcher.unwrap(),
            &start_brackets,
            &end_brackets,
            &bracket_persists,
            parse_context,
            true,
        )?;

        matched_idx = bracket_match.span.end;
        child_matches.push(bracket_match);
    }
}

pub fn greedy_match(
    segments: &[ErasedSegment],
    idx: u32,
    parse_context: &mut ParseContext,
    matchers: &[Matchable],
    include_terminator: bool,
    nested_match: bool,
) -> Result<MatchResult, SQLParseError> {
    let mut working_idx = idx;
    let mut stop_idx: u32;
    let mut child_matches = Vec::new();
    let mut matched;

    loop {
        let (match_result, matcher, inner_matches) =
            parse_context.deeper_match(false, &[], |ctx| {
                next_ex_bracket_match(segments, working_idx, matchers, ctx, "bracket_pairs")
            })?;

        matched = match_result;

        if nested_match {
            child_matches.extend(inner_matches);
        }

        if !matched.has_match() {
            return Ok(MatchResult {
                span: Span {
                    start: idx,
                    end: segments.len() as u32,
                },
                matched: None,
                insert_segments: Vec::new(),
                child_matches,
            });
        }

        let start_idx = matched.span.start;
        stop_idx = matched.span.end;

        let matcher = matcher.unwrap();
        let (strings, types) = matcher.simple(parse_context, None).unwrap();

        if types.is_empty() && strings.iter().all(|s| s.chars().all(|c| c.is_alphabetic())) {
            let mut allowable_match = start_idx == working_idx;

            for idx in (working_idx..=start_idx).rev() {
                if segments[idx as usize - 1].is_meta() {
                    continue;
                }

                allowable_match = matches!(
                    segments[idx as usize - 1].get_type(),
                    SyntaxKind::Whitespace | SyntaxKind::Newline
                );

                break;
            }

            if !allowable_match {
                working_idx = stop_idx;
                continue;
            }
        }

        break;
    }

    if include_terminator {
        return Ok(MatchResult {
            span: Span {
                start: idx,
                end: stop_idx,
            },
            ..MatchResult::default()
        });
    }

    let stop_idx = skip_stop_index_backward_to_code(segments, matched.span.start, idx);

    let span = if idx == stop_idx {
        Span {
            start: idx,
            end: matched.span.start,
        }
    } else {
        Span {
            start: idx,
            end: stop_idx,
        }
    };

    Ok(MatchResult {
        span,
        child_matches,
        ..Default::default()
    })
}

pub fn trim_to_terminator(
    segments: &[ErasedSegment],
    idx: u32,
    terminators: &[Matchable],
    parse_context: &mut ParseContext,
) -> Result<u32, SQLParseError> {
    if idx >= segments.len() as u32 {
        return Ok(segments.len() as u32);
    }

    let early_return = parse_context.deeper_match(false, &[], |ctx| {
        let pruned_terms = prune_options(terminators, segments, ctx, idx);

        for term in pruned_terms {
            if term.match_segments(segments, idx, ctx)?.has_match() {
                return Ok(Some(idx));
            }
        }

        Ok(None)
    })?;

    if let Some(idx) = early_return {
        return Ok(idx);
    }

    let term_match = parse_context.deeper_match(false, &[], |ctx| {
        greedy_match(segments, idx, ctx, terminators, false, false)
    })?;

    Ok(skip_stop_index_backward_to_code(
        segments,
        term_match.span.end,
        idx,
    ))
}
