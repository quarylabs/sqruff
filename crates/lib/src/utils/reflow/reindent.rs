use std::borrow::Cow;
use std::mem::take;

use ahash::{AHashMap, AHashSet};
use itertools::{enumerate, Itertools};

use super::elements::{ReflowElement, ReflowPoint, ReflowSequenceType};
use super::rebreak::RebreakSpan;
use crate::core::rules::base::{LintFix, LintResult};
use crate::helpers::skip_last;
use crate::utils::reflow::elements::IndentStats;

fn has_untemplated_newline(point: &ReflowPoint) -> bool {
    if !point.class_types().into_iter().any(|x| x == "newline" || x == "placeholder") {
        return false;
    }

    for seg in &point.segments {
        if seg.is_type("newline")
            && (seg.get_position_marker().is_none()
                || seg.get_position_marker().unwrap().is_literal())
        {
            return true;
        }

        // if seg.is_type("placeholder") {
        //     // Safe to assume seg can be treated as TemplateSegment based on
        // context     let template_seg =
        // seg.as_any().downcast_ref::<TemplateSegment>().expect("Expected
        // TemplateSegment");     assert_eq!(template_seg.block_type,
        // "literal", "Expected only literal placeholders in ReflowPoint.");
        //     if template_seg.source_str.contains('\n') {
        //         return true;
        //     }
        // }
    }

    false
}

#[derive(Debug, Clone)]
struct IndentPoint {
    idx: usize,
    indent_impulse: isize,
    indent_trough: isize,
    initial_indent_balance: isize,
    last_line_break_idx: Option<usize>,
    is_line_break: bool,
    untaken_indents: Vec<isize>,
}

impl IndentPoint {
    fn closing_indent_balance(&self) -> isize {
        self.initial_indent_balance + self.indent_impulse
    }
}

#[derive(Debug, Clone)]
struct IndentLine {
    initial_indent_balance: isize,
    indent_points: Vec<IndentPoint>,
}

impl IndentLine {
    fn from_points(indent_points: Vec<IndentPoint>) -> Self {
        let starting_balance = if indent_points.last().unwrap().last_line_break_idx.is_some() {
            indent_points[0].closing_indent_balance()
        } else {
            0
        };

        IndentLine { initial_indent_balance: starting_balance, indent_points }
    }

    fn closing_balance(&self) -> isize {
        self.indent_points.last().unwrap().closing_indent_balance()
    }

    fn opening_balance(&self) -> isize {
        if self.indent_points.last().unwrap().last_line_break_idx.is_none() {
            return 0;
        }

        self.indent_points[0].closing_indent_balance()
    }

    fn desired_indent_units(&self, forced_indents: &[usize]) -> isize {
        let relevant_untaken_indents: usize = if self.indent_points[0].indent_trough != 0 {
            self.indent_points[0]
                .untaken_indents
                .iter()
                .filter(|&&i| {
                    i <= self.initial_indent_balance
                        - (self.indent_points[0].indent_impulse
                            - self.indent_points[0].indent_trough)
                })
                .count()
        } else {
            self.indent_points[0].untaken_indents.len()
        };

        self.initial_indent_balance - relevant_untaken_indents as isize
            + forced_indents.len() as isize
    }
}

impl std::fmt::Display for IndentLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let indent_points_str = self
            .indent_points
            .iter()
            .map(|ip| {
                format!(
                    "iPt@{}({}, {}, {}, {:?}, {}, {:?})",
                    ip.idx,
                    ip.indent_impulse,
                    ip.indent_trough,
                    ip.initial_indent_balance,
                    ip.last_line_break_idx,
                    ip.is_line_break,
                    ip.untaken_indents
                )
            })
            .collect::<Vec<String>>()
            .join(", ");

        write!(f, "IndentLine(iib={}, ipts=[{}])", self.initial_indent_balance, indent_points_str)
    }
}

#[allow(unused_variables, dead_code)]
fn revise_templated_lines(lines: Vec<IndentLine>, elements: ReflowSequenceType) {}

#[allow(unused_variables, dead_code)]
fn revise_comment_lines(lines: Vec<IndentLine>, elements: ReflowSequenceType) {}

pub fn construct_single_indent(indent_unit: &str, tab_space_size: usize) -> Cow<'static, str> {
    match indent_unit {
        "tab" => "\t".into(),
        "space" => " ".repeat(tab_space_size).into(),
        _ => unimplemented!("Expected indent_unit of 'tab' or 'space', instead got {indent_unit}"),
    }
}

fn prune_untaken_indents(
    untaken_indents: Vec<isize>,
    incoming_balance: isize,
    indent_stats: &IndentStats,
    has_newline: bool,
) -> Vec<isize> {
    let new_balance_threshold = if indent_stats.trough < indent_stats.impulse {
        incoming_balance + indent_stats.impulse + indent_stats.trough
    } else {
        incoming_balance + indent_stats.impulse
    };

    let mut pruned_untaken_indents: Vec<_> =
        untaken_indents.iter().cloned().filter(|&x| x <= new_balance_threshold).collect();

    if indent_stats.impulse > indent_stats.trough && !has_newline {
        for i in indent_stats.trough..indent_stats.impulse {
            let indent_val = incoming_balance + i + 1;
            if !indent_stats.implicit_indents.contains(&(indent_val - incoming_balance)) {
                pruned_untaken_indents.push(indent_val);
            }
        }
    }

    pruned_untaken_indents
}

fn update_crawl_balances(
    untaken_indents: Vec<isize>,
    incoming_balance: isize,
    indent_stats: &IndentStats,
    has_newline: bool,
) -> (isize, Vec<isize>) {
    let new_untaken_indents =
        prune_untaken_indents(untaken_indents, incoming_balance, indent_stats, has_newline);
    let new_balance = incoming_balance + indent_stats.impulse;

    (new_balance, new_untaken_indents.into_iter().collect_vec())
}

#[allow(unused_variables)]
fn crawl_indent_points(
    elements: &ReflowSequenceType,
    allow_implicit_indents: bool,
) -> Vec<IndentPoint> {
    let mut acc = Vec::new();

    let mut last_line_break_idx = None;
    let mut indent_balance = 0;
    let mut untaken_indents = Vec::new();
    let mut cached_indent_stats = None;
    let mut cached_point = None;

    for (idx, elem) in enumerate(elements) {
        if let ReflowElement::Point(elem) = elem {
            let mut indent_stats =
                IndentStats::from_combination(cached_indent_stats.clone(), elem.indent_impulse());

            if !indent_stats.implicit_indents.is_empty() {
                unimplemented!()
            }

            // Was there a cache?
            if cached_indent_stats.is_some() {
                let cached_point: &IndentPoint = cached_point.as_ref().unwrap();

                if cached_point.is_line_break {
                    acc.push(IndentPoint {
                        idx: cached_point.idx,
                        indent_impulse: indent_stats.impulse,
                        indent_trough: indent_stats.trough,
                        initial_indent_balance: indent_balance,
                        last_line_break_idx: cached_point.last_line_break_idx,
                        is_line_break: true,
                        untaken_indents: take(&mut untaken_indents),
                    });
                    // Before zeroing, crystallise any effect on overall
                    // balances.

                    (indent_balance, untaken_indents) =
                        update_crawl_balances(untaken_indents, indent_balance, &indent_stats, true);

                    let implicit_indents = take(&mut indent_stats.implicit_indents);
                    indent_stats = IndentStats { impulse: 0, trough: 0, implicit_indents };
                } else {
                    // FIXME:
                    // unimplemented!()
                }
            }

            // Reset caches.
            cached_indent_stats = None;
            cached_point = None;

            // Do we have a newline?
            let has_newline = has_untemplated_newline(elem) && Some(idx) != last_line_break_idx;

            // Construct the point we may yield
            let indent_point = IndentPoint {
                idx,
                indent_impulse: indent_stats.impulse,
                indent_trough: indent_stats.trough,
                initial_indent_balance: indent_balance,
                last_line_break_idx,
                is_line_break: has_newline,
                untaken_indents: untaken_indents.clone(),
            };

            if has_newline {
                last_line_break_idx = idx.into();
            }

            if elements[idx + 1].class_types1().contains("comment") {
                cached_indent_stats = indent_stats.clone().into();
                cached_point = indent_point.clone().into();

                continue;
            } else if has_newline
                || indent_stats.impulse != 0
                || indent_stats.trough != 0
                || idx == 0
                || elements[idx + 1].segments()[0].is_type("end_of_file")
            {
                acc.push(indent_point);
            }

            (indent_balance, untaken_indents) =
                update_crawl_balances(untaken_indents, indent_balance, &indent_stats, has_newline);
        }
    }

    acc
}

#[allow(unused_variables)]
fn map_line_buffers(
    elements: &ReflowSequenceType,
    allow_implicit_indents: bool,
) -> (Vec<IndentLine>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut point_buffer = Vec::new();
    let mut previous_points = AHashMap::new();
    let mut untaken_indent_locs = AHashMap::new();
    let imbalanced_locs = Vec::new();

    for indent_point in crawl_indent_points(elements, allow_implicit_indents) {
        point_buffer.push(indent_point.clone());
        previous_points.insert(indent_point.idx, indent_point.clone());

        if !indent_point.is_line_break {
            let indent_stats = elements[indent_point.idx].as_point().unwrap().indent_impulse();
            if indent_point.indent_impulse > indent_point.indent_trough
                && !allow_implicit_indents
                && !indent_stats.implicit_indents.is_empty()
            {
                untaken_indent_locs.insert(
                    indent_point.initial_indent_balance + indent_point.indent_impulse,
                    indent_point.idx,
                );
            }

            continue;
        }

        lines.push(IndentLine::from_points(point_buffer));

        point_buffer = vec![indent_point];
    }

    if point_buffer.len() > 1 {
        lines.push(IndentLine::from_points(point_buffer));
    }

    (lines, imbalanced_locs)
}

fn deduce_line_current_indent(
    elements: &ReflowSequenceType,
    last_line_break_idx: Option<usize>,
) -> Cow<'static, str> {
    let mut indent_seg = None;

    if elements[0].segments().is_empty() {
        return "".into();
    } else if let Some(last_line_break_idx) = last_line_break_idx {
        indent_seg = elements[last_line_break_idx].as_point().unwrap().get_indent_segment();
    } else if matches!(elements[0], ReflowElement::Point(_))
        && elements[0].segments()[0]
            .get_position_marker()
            .map_or(false, |marker| marker.working_loc() == (1, 1))
    {
        if elements[0].segments()[0].is_type("placeholder") {
            unimplemented!()
        } else {
            for segment in elements[0].segments().iter().rev() {
                if segment.is_type("whitespace") && !segment.is_templated() {
                    indent_seg = Some(segment.clone());
                    break;
                }
            }

            if let Some(ref seg) = indent_seg {
                if !seg.is_type("whitespace") {
                    indent_seg = None;
                }
            }
        }
    }

    let Some(indent_seg) = indent_seg else {
        return "".into();
    };

    if indent_seg.is_type("placeholder") {
        unimplemented!()
    } else if indent_seg.get_position_marker().is_none() || !indent_seg.is_templated() {
        return indent_seg.get_raw().unwrap().into();
    } else {
        unimplemented!()
    }
}

fn lint_line_starting_indent(
    elements: &mut ReflowSequenceType,
    indent_line: &IndentLine,
    single_indent: &str,
    forced_indents: &[usize],
) -> Vec<LintResult> {
    let indent_points = &indent_line.indent_points;
    // Set up the default anchor
    let initial_point_idx = indent_points[0].idx;
    let before = elements[initial_point_idx + 1].segments()[0].clone();
    // Find initial indent, and deduce appropriate string indent.
    let current_indent =
        deduce_line_current_indent(elements, indent_points.last().unwrap().last_line_break_idx);
    let initial_point = elements[initial_point_idx].as_point().unwrap();
    let desired_indent_units = indent_line.desired_indent_units(forced_indents);
    let desired_starting_indent = single_indent.repeat(desired_indent_units.try_into().unwrap());

    if current_indent == desired_starting_indent {
        return Vec::new();
    }

    if initial_point_idx > 0 && initial_point_idx < elements.len() - 1 {
        if elements[initial_point_idx + 1].class_types1().contains("comment") {
            let last_indent =
                deduce_line_current_indent(elements, indent_points[0].last_line_break_idx);
            if current_indent.len() == last_indent.len() {
                return Vec::new();
            }
        }

        if elements[initial_point_idx - 1].class_types1().contains("block_comment")
            && elements[initial_point_idx + 1].class_types1().contains("block_comment")
        {
            return Vec::new();
        }
    }

    let (new_results, new_point) = if indent_points[0].idx == 0 && !indent_points[0].is_line_break {
        let init_seg = &elements[indent_points[0].idx].segments()[0];
        let fixes = if init_seg.is_type("placeholder") {
            unimplemented!()
        } else {
            initial_point.segments.clone().into_iter().map(LintFix::delete).collect_vec()
        };

        (
            vec![LintResult::new(
                initial_point.segments[0].clone_box().into(),
                fixes,
                None,
                Some("First line should not be indented.".into()),
                None,
            )],
            ReflowPoint::new(Vec::new()),
        )
    } else {
        initial_point.indent_to(&desired_starting_indent, None, before.into(), None, None)
    };

    elements[initial_point_idx] = new_point.into();

    new_results
}

#[allow(unused_variables, dead_code)]
fn lint_line_untaken_positive_indents(
    elements: Vec<ReflowElement>,
    indent_line: IndentLine,
    single_indent: &str,
    imbalanced_indent_locs: Vec<i32>,
) -> (Vec<LintResult>, Vec<i32>) {
    unimplemented!()
}

#[allow(unused_variables)]
fn lint_line_untaken_negative_indents(
    elements: &mut ReflowSequenceType,
    indent_line: &IndentLine,
    single_indent: &str,
    forced_indents: &[usize],
) -> Vec<LintResult> {
    if indent_line.closing_balance() >= indent_line.opening_balance() {
        return Vec::new();
    }

    for ip in skip_last(indent_line.indent_points.iter()) {
        if ip.is_line_break || ip.indent_impulse >= 0 {
            continue;
        }

        if ip.initial_indent_balance + ip.indent_trough >= indent_line.opening_balance() {
            continue;
        }

        let covered_indents: AHashSet<isize> =
            (ip.initial_indent_balance..=ip.initial_indent_balance + ip.indent_trough).collect();

        let untaken_indents: AHashSet<_> = ip
            .untaken_indents
            .iter()
            .copied()
            .collect::<AHashSet<_>>()
            .difference(&forced_indents.iter().map(|it| *it as isize).collect())
            .copied()
            .collect();

        if covered_indents.is_subset(&untaken_indents) {
            continue;
        }
    }

    Vec::new()
}

#[allow(unused_variables)]
fn lint_line_buffer_indents(
    elements: &mut ReflowSequenceType,
    indent_line: IndentLine,
    single_indent: &str,
    forced_indents: &[usize],
    imbalanced_indent_locs: &[usize],
) -> Vec<LintResult> {
    let mut results = Vec::new();

    results.extend(lint_line_starting_indent(
        elements,
        &indent_line,
        single_indent,
        forced_indents,
    ));

    results.extend(lint_line_untaken_negative_indents(
        elements,
        &indent_line,
        single_indent,
        forced_indents,
    ));

    results
}

#[allow(unused_variables)]
pub fn lint_indent_points(
    elements: ReflowSequenceType,
    single_indent: &str,
    skip_indentation_in: AHashSet<String>,
    allow_implicit_indents: bool,
) -> (ReflowSequenceType, Vec<LintResult>) {
    let (lines, imbalanced_indent_locs) = map_line_buffers(&elements, allow_implicit_indents);

    let mut results = Vec::new();
    let mut elem_buffer = elements.clone();
    for line in lines {
        let line_results = lint_line_buffer_indents(
            &mut elem_buffer,
            line,
            single_indent,
            &[],
            &imbalanced_indent_locs,
        );

        results.extend(line_results);
    }

    (elem_buffer, results)
}

#[allow(unused_variables, dead_code)]
fn source_char_len(elements: Vec<ReflowElement>) -> usize {
    unimplemented!()
}

#[allow(unused_variables, dead_code)]
fn rebreak_priorities(spans: Vec<RebreakSpan>) -> AHashMap<usize, usize> {
    unimplemented!()
}

type MatchedIndentsType = AHashMap<f64, Vec<i32>>;

#[allow(unused_variables, dead_code)]
fn increment_balance(
    input_balance: i32,
    indent_stats: (),
    elem_idx: i32,
) -> (i32, MatchedIndentsType) {
    unimplemented!()
}

#[allow(unused_variables, dead_code)]
fn match_indents(
    line_elements: ReflowSequenceType,
    rebreak_priorities: AHashMap<i32, i32>,
    newline_idx: i32,
    allow_implicit_indents: bool,
) -> MatchedIndentsType {
    unimplemented!()
}

#[allow(unused_variables, dead_code)]
fn fix_long_line_with_comment(
    line_buffer: ReflowSequenceType,
    elements: ReflowSequenceType,
    current_indent: &str,
    line_length_limit: i32,
    last_indent_idx: Option<usize>,
    trailing_comments: &str,
) -> (ReflowSequenceType, Vec<LintFix>) {
    unimplemented!()
}

#[allow(unused_variables, dead_code)]
fn fix_long_line_with_fractional_targets(
    elements: Vec<ReflowElement>,
    target_breaks: Vec<usize>,
    desired_indent: &str,
) -> Vec<LintResult> {
    unimplemented!()
}

#[allow(unused_variables, dead_code)]
fn fix_long_line_with_integer_targets(
    elements: Vec<ReflowElement>,
    target_breaks: Vec<usize>,
    line_length_limit: i32,
    inner_indent: &str,
    outer_indent: &str,
) -> Vec<LintResult> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{IndentLine, IndentPoint};
    use crate::core::parser::segments::test_functions::parse_ansi_string;
    use crate::utils::reflow::sequence::ReflowSequence;

    #[test]
    fn test_reflow__point_get_indent() {
        let cases = [
            ("select 1", 1, None),
            ("select\n  1", 1, "  ".into()),
            ("select\n \n  \n   1", 1, "   ".into()),
        ];

        for (raw_sql_in, elem_idx, indent_out) in cases {
            let root = parse_ansi_string(raw_sql_in);
            let seq = ReflowSequence::from_root(root, &<_>::default());
            let elem = seq.elements()[elem_idx].as_point().unwrap();

            assert_eq!(indent_out, elem.get_indent().as_deref());
        }
    }

    #[test]
    fn test_reflow__desired_indent_units() {
        let cases: [(IndentLine, &[usize], isize); 7] = [
            // Trivial case of a first line.
            (
                IndentLine {
                    initial_indent_balance: 0,
                    indent_points: vec![IndentPoint {
                        idx: 0,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 0,
                        last_line_break_idx: None,
                        is_line_break: false,
                        untaken_indents: Vec::new(),
                    }],
                },
                &[],
                0,
            ),
            // Simple cases of a normal lines.
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 3,
                        last_line_break_idx: 1.into(),
                        is_line_break: true,
                        untaken_indents: Vec::new(),
                    }],
                },
                &[],
                3,
            ),
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 3,
                        last_line_break_idx: Some(1),
                        is_line_break: true,
                        untaken_indents: vec![1],
                    }],
                },
                &[],
                2,
            ),
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 3,
                        last_line_break_idx: Some(1),
                        is_line_break: true,
                        untaken_indents: vec![1, 2],
                    }],
                },
                &[],
                1,
            ),
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 3,
                        last_line_break_idx: Some(1),
                        is_line_break: true,
                        untaken_indents: vec![2],
                    }],
                },
                &[2], // Forced indent takes us back up.
                3,
            ),
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: 3,
                        last_line_break_idx: Some(1),
                        is_line_break: true,
                        untaken_indents: vec![3],
                    }],
                },
                &[],
                2,
            ),
            (
                IndentLine {
                    initial_indent_balance: 3,
                    indent_points: vec![IndentPoint {
                        idx: 6,
                        indent_impulse: 0,
                        indent_trough: -1,
                        initial_indent_balance: 3,
                        last_line_break_idx: Some(1),
                        is_line_break: true,
                        untaken_indents: vec![3],
                    }],
                },
                &[],
                3,
            ),
        ];

        for (indent_line, forced_indents, expected_units) in cases {
            assert_eq!(indent_line.desired_indent_units(forced_indents), expected_units);
        }
    }
}
