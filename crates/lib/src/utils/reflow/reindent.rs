use std::borrow::Cow;
use std::mem::take;

use ahash::{AHashMap, AHashSet};
use itertools::{Itertools, chain, enumerate};
use smol_str::SmolStr;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use strum_macros::EnumString;

use super::elements::{ReflowBlock, ReflowElement, ReflowPoint, ReflowSequenceType};
use super::helpers::fixes_from_results;
use super::rebreak::{LinePosition, RebreakSpan, identify_rebreak_spans};
use crate::core::rules::base::LintResult;
use crate::utils::reflow::elements::IndentStats;

fn has_untemplated_newline(point: &ReflowPoint) -> bool {
    if !point
        .class_types()
        .intersects(const { &SyntaxSet::new(&[SyntaxKind::Newline, SyntaxKind::Placeholder]) })
    {
        return false;
    }
    point.segments().iter().any(|segment| {
        segment.is_type(SyntaxKind::Newline)
            && (segment
                .get_position_marker()
                .is_none_or(|position_marker| position_marker.is_literal()))
    })
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
    pub(crate) fn is_all_comments(&self, elements: &ReflowSequenceType) -> bool {
        self.block_segments(elements).all(|seg| {
            matches!(
                seg.get_type(),
                SyntaxKind::InlineComment | SyntaxKind::BlockComment | SyntaxKind::Comment
            )
        })
    }

    fn block_segments<'a>(
        &self,
        elements: &'a ReflowSequenceType,
    ) -> impl Iterator<Item = &'a ErasedSegment> {
        self.blocks(elements).map(|it| it.segment())
    }

    fn blocks<'a>(
        &self,
        elements: &'a ReflowSequenceType,
    ) -> impl Iterator<Item = &'a ReflowBlock> {
        let slice = if self
            .indent_points
            .last()
            .unwrap()
            .last_line_break_idx
            .is_none()
        {
            0..self.indent_points.last().unwrap().idx
        } else {
            self.indent_points.first().unwrap().idx..self.indent_points.last().unwrap().idx
        };

        elements[slice].iter().filter_map(ReflowElement::as_block)
    }
}

impl IndentLine {
    fn from_points(indent_points: Vec<IndentPoint>) -> Self {
        let starting_balance = if indent_points.last().unwrap().last_line_break_idx.is_some() {
            indent_points[0].closing_indent_balance()
        } else {
            0
        };

        IndentLine {
            initial_indent_balance: starting_balance,
            indent_points,
        }
    }

    fn closing_balance(&self) -> isize {
        self.indent_points.last().unwrap().closing_indent_balance()
    }

    fn opening_balance(&self) -> isize {
        if self
            .indent_points
            .last()
            .unwrap()
            .last_line_break_idx
            .is_none()
        {
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

        write!(
            f,
            "IndentLine(iib={}, ipts=[{}])",
            self.initial_indent_balance, indent_points_str
        )
    }
}

fn revise_comment_lines(lines: &mut [IndentLine], elements: &ReflowSequenceType) {
    let mut comment_line_buffer = Vec::new();
    let mut changes = Vec::new();

    for (idx, line) in enumerate(&mut *lines) {
        if line.is_all_comments(elements) {
            comment_line_buffer.push(idx);
        } else {
            for comment_line_idx in comment_line_buffer.drain(..) {
                changes.push((comment_line_idx, line.initial_indent_balance));
            }
        }
    }

    let changes = changes.into_iter().chain(
        comment_line_buffer
            .into_iter()
            .map(|comment_line_idx| (comment_line_idx, 0)),
    );
    for (comment_line_idx, initial_indent_balance) in changes {
        lines[comment_line_idx].initial_indent_balance = initial_indent_balance;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndentUnit {
    Tab,
    Space(usize),
}

impl Default for IndentUnit {
    fn default() -> Self {
        IndentUnit::Space(4)
    }
}

impl IndentUnit {
    pub fn from_type_and_size(indent_type: &str, indent_size: usize) -> Self {
        match indent_type {
            "tab" => IndentUnit::Tab,
            "space" => IndentUnit::Space(indent_size),
            _ => unreachable!("Invalid indent type {}", indent_type),
        }
    }
}

pub fn construct_single_indent(indent_unit: IndentUnit) -> Cow<'static, str> {
    match indent_unit {
        IndentUnit::Tab => "\t".into(),
        IndentUnit::Space(space_size) => " ".repeat(space_size).into(),
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

    let mut pruned_untaken_indents: Vec<_> = untaken_indents
        .iter()
        .filter(|&x| x <= &new_balance_threshold)
        .copied()
        .collect();

    if indent_stats.impulse > indent_stats.trough && !has_newline {
        for i in indent_stats.trough..indent_stats.impulse {
            let indent_val = incoming_balance + i + 1;

            if !indent_stats
                .implicit_indents
                .contains(&(indent_val - incoming_balance))
            {
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

    (new_balance, new_untaken_indents)
}

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
                let mut unclosed_bracket = false;

                if allow_implicit_indents
                    && elements[idx + 1]
                        .class_types()
                        .contains(SyntaxKind::StartBracket)
                {
                    let depth = elements[idx + 1]
                        .as_block()
                        .unwrap()
                        .depth_info()
                        .stack_depth;

                    let elems = &elements[idx + 1..];
                    unclosed_bracket = elems.is_empty();

                    for elem_j in elems {
                        if let Some(elem_j) = elem_j.as_point() {
                            if elem_j.num_newlines() > 0 {
                                unclosed_bracket = true;
                                break;
                            }
                        } else if elem_j.class_types().contains(SyntaxKind::EndBracket)
                            && elem_j.as_block().unwrap().depth_info().stack_depth == depth
                        {
                            unclosed_bracket = false;
                            break;
                        } else {
                            unclosed_bracket = true;
                        }
                    }
                }

                if unclosed_bracket || !allow_implicit_indents {
                    indent_stats.implicit_indents = Default::default();
                }
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
                    indent_stats = IndentStats {
                        impulse: 0,
                        trough: 0,
                        implicit_indents,
                    };
                } else {
                    acc.push(IndentPoint {
                        idx: cached_point.idx,
                        indent_impulse: 0,
                        indent_trough: 0,
                        initial_indent_balance: indent_balance,
                        last_line_break_idx: cached_point.last_line_break_idx,
                        is_line_break: false,
                        untaken_indents: untaken_indents.clone(),
                    });
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

            if elements[idx + 1].class_types().intersects(
                const {
                    &SyntaxSet::new(&[
                        SyntaxKind::Comment,
                        SyntaxKind::InlineComment,
                        SyntaxKind::BlockComment,
                    ])
                },
            ) {
                cached_indent_stats = indent_stats.clone().into();
                cached_point = indent_point.clone().into();

                continue;
            } else if has_newline
                || indent_stats.impulse != 0
                || indent_stats.trough != 0
                || idx == 0
                || elements[idx + 1].segments()[0].is_type(SyntaxKind::EndOfFile)
            {
                acc.push(indent_point);
            }

            (indent_balance, untaken_indents) =
                update_crawl_balances(untaken_indents, indent_balance, &indent_stats, has_newline);
        }
    }

    acc
}

fn map_line_buffers(
    elements: &ReflowSequenceType,
    allow_implicit_indents: bool,
) -> (Vec<IndentLine>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut point_buffer = Vec::new();
    let mut previous_points = AHashMap::new();
    let mut untaken_indent_locs = AHashMap::new();
    let mut imbalanced_locs = Vec::new();

    for indent_point in crawl_indent_points(elements, allow_implicit_indents) {
        point_buffer.push(indent_point.clone());
        previous_points.insert(indent_point.idx, indent_point.clone());

        if !indent_point.is_line_break {
            let indent_stats = elements[indent_point.idx]
                .as_point()
                .unwrap()
                .indent_impulse();

            if (indent_stats.implicit_indents.is_empty() || !allow_implicit_indents)
                && indent_point.indent_impulse > indent_point.indent_trough
            {
                untaken_indent_locs.insert(
                    indent_point.initial_indent_balance + indent_point.indent_impulse,
                    indent_point.idx,
                );
            }

            continue;
        }

        lines.push(IndentLine::from_points(point_buffer.clone()));

        let following_class_types = elements[indent_point.idx + 1].class_types();
        if indent_point.indent_trough != 0 && !following_class_types.contains(SyntaxKind::EndOfFile)
        {
            let passing_indents = Range::new(
                indent_point.initial_indent_balance,
                indent_point.initial_indent_balance + indent_point.indent_trough,
                -1,
            )
            .reversed();

            for i in passing_indents {
                let Some(&loc) = untaken_indent_locs.get(&i) else {
                    break;
                };

                if elements[loc + 1]
                    .class_types()
                    .contains(SyntaxKind::StartBracket)
                {
                    continue;
                }

                if point_buffer.iter().any(|ip| ip.idx == loc) {
                    continue;
                }

                let mut _pt = None;
                for j in loc..indent_point.idx {
                    if let Some(pt) = previous_points.get(&j) {
                        if pt.is_line_break {
                            _pt = Some(pt);
                            break;
                        }
                    }
                }

                let _pt = _pt.unwrap();

                // Then check if all comments.
                if (_pt.idx + 1..indent_point.idx).step_by(2).all(|k| {
                    elements[k].class_types().intersects(
                        const {
                            &SyntaxSet::new(&[
                                SyntaxKind::Comment,
                                SyntaxKind::InlineComment,
                                SyntaxKind::BlockComment,
                            ])
                        },
                    )
                }) {
                    // It is all comments. Ignore it.
                    continue;
                }

                imbalanced_locs.push(loc);
            }
        }

        untaken_indent_locs
            .retain(|&k, _| k <= indent_point.initial_indent_balance + indent_point.indent_trough);
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
) -> SmolStr {
    let mut indent_seg = None;

    if elements[0].segments().is_empty() {
        return "".into();
    } else if let Some(last_line_break_idx) = last_line_break_idx {
        indent_seg = elements[last_line_break_idx]
            .as_point()
            .unwrap()
            .get_indent_segment();
    } else if matches!(elements[0], ReflowElement::Point(_))
        && elements[0].segments()[0]
            .get_position_marker()
            .is_some_and(|marker| marker.working_loc() == (1, 1))
    {
        if elements[0].segments()[0].is_type(SyntaxKind::Placeholder) {
            unimplemented!()
        } else {
            for segment in elements[0].segments().iter().rev() {
                if segment.is_type(SyntaxKind::Whitespace) && !segment.is_templated() {
                    indent_seg = Some(segment.clone());
                    break;
                }
            }

            if let Some(ref seg) = indent_seg {
                if !seg.is_type(SyntaxKind::Whitespace) {
                    indent_seg = None;
                }
            }
        }
    }

    let Some(indent_seg) = indent_seg else {
        return "".into();
    };

    if indent_seg.is_type(SyntaxKind::Placeholder) {
        unimplemented!()
    } else if indent_seg.get_position_marker().is_none() || !indent_seg.is_templated() {
        return indent_seg.raw().clone();
    } else {
        unimplemented!()
    }
}

fn lint_line_starting_indent(
    tables: &Tables,
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
    let desired_starting_indent = desired_indent_units
        .try_into()
        .map_or(String::new(), |n| single_indent.repeat(n));

    if current_indent == desired_starting_indent {
        return Vec::new();
    }

    if initial_point_idx > 0 && initial_point_idx < elements.len() - 1 {
        if elements[initial_point_idx + 1].class_types().intersects(
            const {
                &SyntaxSet::new(&[
                    SyntaxKind::Comment,
                    SyntaxKind::BlockComment,
                    SyntaxKind::InlineComment,
                ])
            },
        ) {
            let last_indent =
                deduce_line_current_indent(elements, indent_points[0].last_line_break_idx);

            if current_indent.len() == last_indent.len() {
                return Vec::new();
            }
        }

        if elements[initial_point_idx - 1]
            .class_types()
            .contains(SyntaxKind::BlockComment)
            && elements[initial_point_idx + 1]
                .class_types()
                .contains(SyntaxKind::BlockComment)
            && current_indent.len() > desired_starting_indent.len()
        {
            return Vec::new();
        }
    }

    let (new_results, new_point) = if indent_points[0].idx == 0 && !indent_points[0].is_line_break {
        let init_seg = &elements[indent_points[0].idx].segments()[0];
        let fixes = if init_seg.is_type(SyntaxKind::Placeholder) {
            unimplemented!()
        } else {
            initial_point
                .segments()
                .iter()
                .cloned()
                .map(LintFix::delete)
                .collect_vec()
        };

        (
            vec![LintResult::new(
                initial_point.segments()[0].clone().into(),
                fixes,
                Some("First line should not be indented.".into()),
                None,
            )],
            ReflowPoint::new(Vec::new()),
        )
    } else {
        initial_point.indent_to(
            tables,
            &desired_starting_indent,
            None,
            before.into(),
            None,
            None,
        )
    };

    elements[initial_point_idx] = new_point.into();

    new_results
}

fn lint_line_untaken_positive_indents(
    tables: &Tables,
    elements: &mut [ReflowElement],
    indent_line: &IndentLine,
    single_indent: &str,
    imbalanced_indent_locs: &[usize],
) -> (Vec<LintResult>, Vec<usize>) {
    // First check whether this line contains any of the untaken problem points.
    for ip in &indent_line.indent_points {
        if imbalanced_indent_locs.contains(&ip.idx) {
            // Force it at the relevant position.
            let desired_indent = single_indent
                .repeat((ip.closing_indent_balance() - ip.untaken_indents.len() as isize) as usize);
            let target_point = elements[ip.idx].as_point().unwrap();

            let (results, new_point) = target_point.indent_to(
                tables,
                &desired_indent,
                None,
                Some(elements[ip.idx + 1].segments()[0].clone()),
                Some("reflow.indent.imbalance"),
                None,
            );

            elements[ip.idx] = ReflowElement::Point(new_point);
            // Keep track of the indent we forced, by returning it.
            return (results, vec![ip.closing_indent_balance() as usize]);
        }
    }

    // If we don't close the line higher there won't be any.
    let starting_balance = indent_line.opening_balance();
    let last_ip = indent_line.indent_points.last().unwrap();
    // Check whether it closes the opening indent.
    if last_ip.initial_indent_balance + last_ip.indent_trough <= starting_balance {
        return (vec![], vec![]);
    }

    // Account for the closing trough.
    let mut closing_trough = last_ip.initial_indent_balance
        + if last_ip.indent_trough == 0 {
            last_ip.indent_impulse
        } else {
            last_ip.indent_trough
        };

    // Edge case: Adjust closing trough for trailing indents after comments
    // disrupting closing trough.
    let mut _bal = 0;
    for elem in &elements[last_ip.idx + 1..] {
        if let ReflowElement::Point(_) = elem {
            let stats = elem.as_point().unwrap().indent_impulse();
            // If it's positive, stop. We likely won't find enough negative to come.
            if stats.impulse > 0 {
                break;
            }
            closing_trough = _bal + stats.trough;
            _bal += stats.impulse;
        } else if !elem.class_types().intersects(
            const {
                &SyntaxSet::new(&[
                    SyntaxKind::Comment,
                    SyntaxKind::InlineComment,
                    SyntaxKind::BlockComment,
                ])
            },
        ) {
            break;
        }
    }

    // On the way up we're looking for whether the ending balance was an untaken
    // indent or not.
    if !indent_line
        .indent_points
        .last()
        .unwrap()
        .untaken_indents
        .contains(&closing_trough)
    {
        // If the closing point doesn't correspond to an untaken indent within the line
        // (i.e. it _was_ taken), then there won't be an appropriate place to
        // force an indent.
        return (vec![], vec![]);
    }

    // The closing indent balance *does* correspond to an untaken indent on this
    // line. We *should* force a newline at that position.
    let mut target_point_idx = 0;
    let mut desired_indent = String::new();
    for ip in &indent_line.indent_points {
        if ip.closing_indent_balance() == closing_trough {
            target_point_idx = ip.idx;
            desired_indent = single_indent
                .repeat((ip.closing_indent_balance() - ip.untaken_indents.len() as isize) as usize);
            break;
        }
    }

    let target_point = elements[target_point_idx].as_point().unwrap();

    let (results, new_point) = target_point.indent_to(
        tables,
        &desired_indent,
        None,
        Some(elements[target_point_idx + 1].segments()[0].clone()),
        Some("reflow.indent.positive"),
        None,
    );

    elements[target_point_idx] = ReflowElement::Point(new_point);
    // Keep track of the indent we forced, by returning it.
    (results, vec![closing_trough as usize])
}

fn lint_line_untaken_negative_indents(
    tables: &Tables,
    elements: &mut ReflowSequenceType,
    indent_line: &IndentLine,
    single_indent: &str,
    forced_indents: &[usize],
) -> Vec<LintResult> {
    let mut results = Vec::new();

    if indent_line.closing_balance() >= indent_line.opening_balance() {
        return Vec::new();
    }

    for ip in indent_line.indent_points.split_last().unwrap().1 {
        if ip.is_line_break || ip.indent_impulse >= 0 {
            continue;
        }

        if ip.initial_indent_balance + ip.indent_trough >= indent_line.opening_balance() {
            continue;
        }

        let covered_indents: AHashSet<isize> = Range::new(
            ip.initial_indent_balance,
            ip.initial_indent_balance + ip.indent_trough,
            -1,
        )
        .collect();

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

        if elements.get(ip.idx + 1).is_some_and(|elem| {
            elem.class_types().intersects(
                const { &SyntaxSet::new(&[SyntaxKind::StatementTerminator, SyntaxKind::Comma]) },
            )
        }) {
            continue;
        }

        let desired_indent = single_indent.repeat(
            (ip.closing_indent_balance() - ip.untaken_indents.len() as isize
                + forced_indents.len() as isize)
                .max(0) as usize,
        );

        let target_point = elements[ip.idx].as_point().unwrap();
        let (mut new_results, new_point) = target_point.indent_to(
            tables,
            &desired_indent,
            None,
            elements[ip.idx + 1].segments()[0].clone().into(),
            None,
            "reflow.indent.negative".into(),
        );
        elements[ip.idx] = new_point.into();
        results.append(&mut new_results);
    }

    results
}

fn lint_line_buffer_indents(
    tables: &Tables,
    elements: &mut ReflowSequenceType,
    indent_line: IndentLine,
    single_indent: &str,
    forced_indents: &mut Vec<usize>,
    imbalanced_indent_locs: &[usize],
) -> Vec<LintResult> {
    let mut results = Vec::new();

    let mut new_results = lint_line_starting_indent(
        tables,
        elements,
        &indent_line,
        single_indent,
        forced_indents,
    );
    results.append(&mut new_results);

    let (mut new_results, mut new_indents) = lint_line_untaken_positive_indents(
        tables,
        elements,
        &indent_line,
        single_indent,
        imbalanced_indent_locs,
    );

    if !new_results.is_empty() {
        results.append(&mut new_results);
        forced_indents.append(&mut new_indents);
        return results;
    }

    results.extend(lint_line_untaken_negative_indents(
        tables,
        elements,
        &indent_line,
        single_indent,
        forced_indents,
    ));

    forced_indents.retain(|&i| (i as isize) < indent_line.closing_balance());

    results
}

pub fn lint_indent_points(
    tables: &Tables,
    elements: ReflowSequenceType,
    single_indent: &str,
    _skip_indentation_in: AHashSet<String>,
    allow_implicit_indents: bool,
) -> (ReflowSequenceType, Vec<LintResult>) {
    let (mut lines, imbalanced_indent_locs) = map_line_buffers(&elements, allow_implicit_indents);

    let mut results = Vec::new();
    let mut elem_buffer = elements.clone();
    let mut forced_indents = Vec::new();

    revise_comment_lines(&mut lines, &elements);

    for line in lines {
        let line_results = lint_line_buffer_indents(
            tables,
            &mut elem_buffer,
            line,
            single_indent,
            &mut forced_indents,
            &imbalanced_indent_locs,
        );

        results.extend(line_results);
    }

    (elem_buffer, results)
}

fn source_char_len(elements: &[ReflowElement]) -> usize {
    let mut char_len = 0;
    let mut last_source_slice = None;

    for seg in elements.iter().flat_map(|elem| elem.segments()) {
        if seg.is_type(SyntaxKind::Indent) || seg.is_type(SyntaxKind::Dedent) {
            continue;
        }

        let Some(pos_marker) = seg.get_position_marker() else {
            break;
        };

        let source_slice = pos_marker.source_slice.clone();
        let source_str = pos_marker.source_str();

        if let Some(pos) = source_str.find('\n') {
            char_len += pos;
            break;
        }

        let slice_len = source_slice.end - source_slice.start;

        if Some(source_slice.clone()) != last_source_slice {
            if !seg.raw().is_empty() && slice_len == 0 {
                char_len += seg.raw().chars().count();
            } else if slice_len == 0 {
                continue;
            } else if pos_marker.is_literal() {
                char_len += seg.raw().chars().count();
                last_source_slice = Some(source_slice);
            } else {
                char_len += source_slice.end - source_slice.start;
                last_source_slice = Some(source_slice);
            }
        }
    }

    char_len
}

fn rebreak_priorities(spans: Vec<RebreakSpan>) -> AHashMap<usize, usize> {
    let mut rebreak_priority = AHashMap::with_capacity(spans.len());

    for span in spans {
        let rebreak_indices: &[usize] = match span.line_position {
            LinePosition::Leading => &[span.start_idx - 1],
            LinePosition::Trailing => &[span.end_idx + 1],
            LinePosition::Alone => &[span.start_idx - 1, span.end_idx + 1],
            _ => {
                unimplemented!()
            }
        };

        let span_raw = span.target.raw().to_uppercase();
        let mut priority = 6;

        if span_raw == "," {
            priority = 1;
        } else if span.target.is_type(SyntaxKind::AssignmentOperator) {
            priority = 2;
        } else if span_raw == "OR" {
            priority = 3;
        } else if span_raw == "AND" {
            priority = 4;
        } else if span.target.is_type(SyntaxKind::ComparisonOperator) {
            priority = 5;
        } else if ["*", "/", "%"].contains(&span_raw.as_str()) {
            priority = 7;
        }

        for rebreak_idx in rebreak_indices {
            rebreak_priority.insert(*rebreak_idx, priority);
        }
    }

    rebreak_priority
}

type MatchedIndentsType = AHashMap<FloatTypeWrapper, Vec<usize>>;

fn increment_balance(
    input_balance: isize,
    indent_stats: &IndentStats,
    elem_idx: usize,
) -> (isize, MatchedIndentsType) {
    let mut balance = input_balance;
    let mut matched_indents = AHashMap::new();

    if indent_stats.trough < 0 {
        for b in (0..indent_stats.trough.abs()).step_by(1) {
            let key = FloatTypeWrapper::new((balance + -b) as f64);
            matched_indents
                .entry(key)
                .or_insert_with(Vec::new)
                .push(elem_idx);
        }
        balance += indent_stats.impulse;
    } else if indent_stats.impulse > 0 {
        for b in 0..indent_stats.impulse {
            let key = FloatTypeWrapper::new((balance + b + 1) as f64);
            matched_indents
                .entry(key)
                .or_insert_with(Vec::new)
                .push(elem_idx);
        }
        balance += indent_stats.impulse;
    }

    (balance, matched_indents)
}

fn match_indents(
    line_elements: ReflowSequenceType,
    rebreak_priorities: AHashMap<usize, usize>,
    newline_idx: usize,
    allow_implicit_indents: bool,
) -> MatchedIndentsType {
    let mut balance = 0;
    let mut matched_indents: MatchedIndentsType = AHashMap::new();
    let mut implicit_indents = AHashMap::new();

    for (idx, e) in enumerate(&line_elements) {
        let ReflowElement::Point(point) = e else {
            continue;
        };

        let indent_stats = point.indent_impulse();

        let e_idx =
            (newline_idx as isize - line_elements.len() as isize + idx as isize + 1) as usize;

        if !indent_stats.implicit_indents.is_empty() {
            implicit_indents.insert(e_idx, indent_stats.implicit_indents.clone());
        }

        let nmi;
        (balance, nmi) = increment_balance(balance, indent_stats, e_idx);
        for (b, indices) in nmi {
            matched_indents.entry(b).or_default().extend(indices);
        }

        let Some(&priority) = rebreak_priorities.get(&idx) else {
            continue;
        };

        let balance = FloatTypeWrapper::new(balance as f64 + 0.5 + (priority as f64 / 100.0));
        matched_indents.entry(balance).or_default().push(e_idx);
    }

    matched_indents.retain(|_key, value| value != &[newline_idx]);

    if allow_implicit_indents {
        let keys: Vec<_> = matched_indents.keys().copied().collect();
        for indent_level in keys {
            let major_points: AHashSet<_> = matched_indents[&indent_level]
                .iter()
                .copied()
                .collect::<AHashSet<_>>()
                .difference(&AHashSet::from([newline_idx]))
                .copied()
                .collect::<AHashSet<_>>()
                .difference(&implicit_indents.keys().copied().collect::<AHashSet<_>>())
                .copied()
                .collect();

            if major_points.is_empty() {
                matched_indents.remove(&indent_level);
            }
        }
    }

    matched_indents
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Eq, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum TrailingComments {
    #[default]
    Before,
    After,
}

fn fix_long_line_with_comment(
    tables: &Tables,
    line_buffer: &ReflowSequenceType,
    elements: &ReflowSequenceType,
    current_indent: &str,
    line_length_limit: usize,
    last_indent_idx: Option<usize>,
    trailing_comments: TrailingComments,
) -> (ReflowSequenceType, Vec<LintFix>) {
    if line_buffer
        .last()
        .unwrap()
        .segments()
        .last()
        .unwrap()
        .raw()
        .contains("noqa")
    {
        return (elements.clone(), Vec::new());
    }

    if line_buffer
        .last()
        .unwrap()
        .segments()
        .last()
        .unwrap()
        .raw()
        .len()
        + current_indent.len()
        > line_length_limit
    {
        return (elements.clone(), Vec::new());
    }

    let comment_seg = line_buffer.last().unwrap().segments().last().unwrap();
    let first_seg = line_buffer.first().unwrap().segments().first().unwrap();
    let last_elem_idx = elements
        .iter()
        .position(|elem| elem == line_buffer.last().unwrap())
        .unwrap();

    if trailing_comments == TrailingComments::After {
        let mut elements = elements.clone();
        let anchor_point = line_buffer[line_buffer.len() - 2].as_point().unwrap();
        let (results, new_point) = anchor_point.indent_to(
            tables,
            current_indent,
            None,
            comment_seg.clone().into(),
            None,
            None,
        );
        elements.splice(
            last_elem_idx - 1..last_elem_idx,
            [new_point.into()].iter().cloned(),
        );
        return (elements, fixes_from_results(results.into_iter()).collect());
    }

    let mut fixes = chain(
        Some(LintFix::delete(comment_seg.clone())),
        line_buffer[line_buffer.len() - 2]
            .segments()
            .iter()
            .filter(|ws| ws.is_type(SyntaxKind::Whitespace))
            .map(|ws| LintFix::delete(ws.clone())),
    )
    .collect_vec();

    let new_point;
    let anchor;
    let prev_elems: Vec<ReflowElement>;

    if let Some(idx) = last_indent_idx {
        new_point = ReflowPoint::new(vec![
            SegmentBuilder::newline(tables.next_id(), "\n"),
            SegmentBuilder::whitespace(tables.next_id(), current_indent),
        ]);
        prev_elems = elements[..=idx].to_vec();
        anchor = elements[idx + 1].segments()[0].clone();
    } else {
        new_point = ReflowPoint::new(vec![SegmentBuilder::newline(tables.next_id(), "\n")]);
        prev_elems = Vec::new();
        anchor = first_seg.clone();
    }

    fixes.push(LintFix::create_before(
        anchor,
        chain(
            Some(comment_seg.clone()),
            new_point.segments().iter().cloned(),
        )
        .collect_vec(),
    ));

    let elements: Vec<_> = prev_elems
        .into_iter()
        .chain(Some(line_buffer.last().unwrap().clone()))
        .chain(Some(new_point.into()))
        .chain(line_buffer.iter().take(line_buffer.len() - 2).cloned())
        .chain(elements.iter().skip(last_elem_idx + 1).cloned())
        .collect();

    (elements, fixes)
}

fn fix_long_line_with_fractional_targets(
    tables: &Tables,
    elements: &mut [ReflowElement],
    target_breaks: Vec<usize>,
    desired_indent: &str,
) -> Vec<LintResult> {
    let mut line_results = Vec::new();

    for e_idx in target_breaks {
        let e = elements[e_idx].as_point().unwrap();
        let (new_results, new_point) = e.indent_to(
            tables,
            desired_indent,
            elements[e_idx - 1].segments().last().cloned(),
            elements[e_idx + 1].segments()[0].clone().into(),
            None,
            None,
        );

        elements[e_idx] = new_point.into();
        line_results.extend(new_results);
    }

    line_results
}

fn fix_long_line_with_integer_targets(
    tables: &Tables,
    elements: &mut [ReflowElement],
    mut target_breaks: Vec<usize>,
    line_length_limit: usize,
    inner_indent: &str,
    outer_indent: &str,
) -> Vec<LintResult> {
    let mut line_results = Vec::new();

    let mut purge_before = 0;
    for &e_idx in &target_breaks {
        let Some(pos_marker) = elements[e_idx + 1].segments()[0].get_position_marker() else {
            break;
        };

        if pos_marker.working_line_pos > line_length_limit {
            break;
        }

        let e = elements[e_idx].as_point().unwrap();
        if e.indent_impulse().trough < 0 {
            continue;
        }

        purge_before = e_idx;
    }

    target_breaks.retain(|&e_idx| e_idx >= purge_before);

    for e_idx in target_breaks {
        let e = elements[e_idx].as_point().unwrap().clone();
        let indent_stats = e.indent_impulse();

        let new_indent = if indent_stats.impulse < 0 {
            if elements[e_idx + 1].class_types().intersects(
                const { &SyntaxSet::new(&[SyntaxKind::StatementTerminator, SyntaxKind::Comma]) },
            ) {
                break;
            }

            outer_indent
        } else {
            inner_indent
        };

        let (new_results, new_point) = e.indent_to(
            tables,
            new_indent,
            elements[e_idx - 1].segments().last().cloned(),
            elements[e_idx + 1].segments().first().cloned(),
            None,
            None,
        );

        elements[e_idx] = new_point.into();
        line_results.extend(new_results);

        if indent_stats.trough < 0 {
            break;
        }
    }

    line_results
}

pub fn lint_line_length(
    tables: &Tables,
    elements: &ReflowSequenceType,
    root_segment: &ErasedSegment,
    single_indent: &str,
    line_length_limit: usize,
    allow_implicit_indents: bool,
    trailing_comments: TrailingComments,
) -> (ReflowSequenceType, Vec<LintResult>) {
    if line_length_limit == 0 {
        return (elements.clone(), Vec::new());
    }

    let mut elem_buffer = elements.clone();
    let mut line_buffer = Vec::new();
    let mut results = Vec::new();

    let mut last_indent_idx = None;
    for (i, elem) in enumerate(elements) {
        if elem
            .as_point()
            .filter(|point| {
                elem_buffer[i + 1]
                    .class_types()
                    .contains(SyntaxKind::EndOfFile)
                    || has_untemplated_newline(point)
            })
            .is_some()
        {
            // In either case we want to process this, so carry on.
        } else {
            line_buffer.push(elem.clone());
            continue;
        }

        if line_buffer.is_empty() {
            continue;
        }

        let current_indent = if let Some(last_indent_idx) = last_indent_idx {
            deduce_line_current_indent(&elem_buffer, Some(last_indent_idx))
        } else {
            "".into()
        };

        let char_len = source_char_len(&line_buffer);
        let line_len = current_indent.len() + char_len;

        let first_seg = line_buffer[0].segments()[0].clone();
        let line_no = first_seg.get_position_marker().unwrap().working_line_no;

        if line_len <= line_length_limit {
            tracing::info!(
                "Line #{}. Length {} <= {}. OK.",
                line_no,
                line_len,
                line_length_limit,
            )
        } else {
            let line_elements = chain(line_buffer.clone(), Some(elem.clone())).collect_vec();
            let mut fixes: Vec<LintFix> = Vec::new();

            let mut combined_elements = line_elements.clone();
            combined_elements.push(elements[i + 1].clone());

            let spans = identify_rebreak_spans(&combined_elements, root_segment);
            let rebreak_priorities = rebreak_priorities(spans);

            let matched_indents =
                match_indents(line_elements, rebreak_priorities, i, allow_implicit_indents);

            let desc = format!("Line is too long ({line_len} > {line_length_limit}).");

            if line_buffer.len() > 1
                && line_buffer
                    .last()
                    .unwrap()
                    .segments()
                    .last()
                    .unwrap()
                    .is_type(SyntaxKind::InlineComment)
            {
                (elem_buffer, fixes) = fix_long_line_with_comment(
                    tables,
                    &line_buffer,
                    elements,
                    &current_indent,
                    line_length_limit,
                    last_indent_idx,
                    trailing_comments,
                );
            } else if matched_indents.is_empty() {
                tracing::debug!("Handling as unfixable line.");
            } else {
                tracing::debug!("Handling as normal line.");
                let target_balance = matched_indents
                    .keys()
                    .map(|k| k.into_f64())
                    .fold(f64::INFINITY, f64::min);
                let mut desired_indent = current_indent.to_string();

                if target_balance >= 1.0 {
                    desired_indent += single_indent;
                }

                let mut target_breaks =
                    matched_indents[&FloatTypeWrapper::new(target_balance)].clone();

                if let Some(pos) = target_breaks.iter().position(|&x| x == i) {
                    target_breaks.remove(pos);
                }

                let line_results = if target_balance % 1.0 == 0.0 {
                    fix_long_line_with_integer_targets(
                        tables,
                        &mut elem_buffer,
                        target_breaks,
                        line_length_limit,
                        &desired_indent,
                        &current_indent,
                    )
                } else {
                    fix_long_line_with_fractional_targets(
                        tables,
                        &mut elem_buffer,
                        target_breaks,
                        &desired_indent,
                    )
                };

                fixes = fixes_from_results(line_results.into_iter()).collect();
            }

            results.push(LintResult::new(first_seg.into(), fixes, desc.into(), None))
        }

        line_buffer.clear();
        last_indent_idx = Some(i);
    }

    (elem_buffer, results)
}

#[derive(Default, Hash, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
struct FloatTypeWrapper(u64);

impl FloatTypeWrapper {
    fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    fn into_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}

impl std::fmt::Debug for FloatTypeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", f64::from_bits(self.0))
    }
}

impl std::fmt::Display for FloatTypeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", f64::from_bits(self.0))
    }
}

#[derive(Clone)]
pub(crate) struct Range {
    index: isize,
    start: isize,
    step: isize,
    length: isize,
}

impl Range {
    pub(crate) fn new(start: isize, stop: isize, step: isize) -> Self {
        Self {
            index: 0,
            start,
            step,
            length: if step.is_negative() && start > stop {
                (start - stop - 1) / (-step) + 1
            } else if start < stop {
                if step.is_positive() && step == 1 {
                    stop - start
                } else {
                    (stop - start - 1) / step + 1
                }
            } else {
                0
            },
        }
    }

    fn reversed(self) -> Self {
        let length = self.length;
        let stop = self.start - self.step;
        let start = stop + length * self.step;
        let step = -self.step;

        Self {
            index: 0,
            start,
            step,
            length,
        }
    }
}

impl Iterator for Range {
    type Item = isize;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        self.index += 1;
        if index < self.length {
            Some(self.start + index * self.step)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use sqruff_lib::core::test_functions::parse_ansi_string;

    use super::{IndentLine, IndentPoint};
    use crate::utils::reflow::sequence::ReflowSequence;

    #[test]
    fn test_reflow_point_get_indent() {
        let cases = [
            ("select 1", 1, None),
            ("select\n  1", 1, "  ".into()),
            ("select\n \n  \n   1", 1, "   ".into()),
        ];

        for (raw_sql_in, elem_idx, indent_out) in cases {
            let root = parse_ansi_string(raw_sql_in);
            let config = <_>::default();
            let seq = ReflowSequence::from_root(root, &config);
            let elem = seq.elements()[elem_idx].as_point().unwrap();

            assert_eq!(indent_out, elem.get_indent().as_deref());
        }
    }

    #[test]
    fn test_reflow_desired_indent_units() {
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
            assert_eq!(
                indent_line.desired_indent_units(forced_indents),
                expected_units
            );
        }
    }
}
