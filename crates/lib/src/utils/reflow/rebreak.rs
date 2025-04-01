use std::cmp::PartialEq;
use std::str::FromStr;

use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::capitalize;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, Tables};
use strum_macros::{AsRefStr, EnumString};

use super::elements::{ReflowElement, ReflowSequenceType};
use crate::core::rules::base::LintResult;
use crate::utils::reflow::depth_map::StackPositionType;
use crate::utils::reflow::elements::ReflowPoint;
use crate::utils::reflow::helpers::{deduce_line_indent, fixes_from_results};

#[derive(Debug)]
pub struct RebreakSpan {
    pub(crate) target: ErasedSegment,
    pub(crate) start_idx: usize,
    pub(crate) end_idx: usize,
    pub(crate) line_position: LinePosition,
    pub(crate) strict: bool,
}

#[derive(Debug)]
pub struct RebreakIndices {
    _dir: i32,
    adj_pt_idx: isize,
    newline_pt_idx: isize,
    pre_code_pt_idx: isize,
}

impl RebreakIndices {
    fn from_elements(elements: &ReflowSequenceType, start_idx: usize, dir: i32) -> Option<Self> {
        assert!(dir == 1 || dir == -1);
        let limit = if dir == -1 { 0 } else { elements.len() };
        let adj_point_idx = start_idx as isize + dir as isize;

        if adj_point_idx < 0 || adj_point_idx >= elements.len() as isize {
            return None;
        }

        let mut newline_point_idx = adj_point_idx;
        while (dir == 1 && newline_point_idx < limit as isize)
            || (dir == -1 && newline_point_idx >= 0)
        {
            if elements[newline_point_idx as usize]
                .class_types()
                .contains(SyntaxKind::Newline)
                || elements[(newline_point_idx + dir as isize) as usize]
                    .segments()
                    .iter()
                    .any(|seg| seg.is_code())
            {
                break;
            }
            newline_point_idx += 2 * dir as isize;
        }

        let mut pre_code_point_idx = newline_point_idx;
        while (dir == 1 && pre_code_point_idx < limit as isize)
            || (dir == -1 && pre_code_point_idx >= 0)
        {
            if elements[(pre_code_point_idx + dir as isize) as usize]
                .segments()
                .iter()
                .any(|seg| seg.is_code())
            {
                break;
            }
            pre_code_point_idx += 2 * dir as isize;
        }

        RebreakIndices {
            _dir: dir,
            adj_pt_idx: adj_point_idx,
            newline_pt_idx: newline_point_idx,
            pre_code_pt_idx: pre_code_point_idx,
        }
        .into()
    }
}

#[derive(Debug)]
pub struct RebreakLocation {
    target: ErasedSegment,
    prev: RebreakIndices,
    next: RebreakIndices,
    line_position: LinePosition,
    strict: bool,
}

#[derive(Debug, PartialEq, Clone, Copy, AsRefStr, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum LinePosition {
    Leading,
    Trailing,
    Alone,
    Strict,
}

impl RebreakLocation {
    /// Expand a span to a location.
    pub fn from_span(span: RebreakSpan, elements: &ReflowSequenceType) -> Option<Self> {
        Self {
            target: span.target,
            prev: RebreakIndices::from_elements(elements, span.start_idx, -1)?,
            next: RebreakIndices::from_elements(elements, span.end_idx, 1)?,
            line_position: span.line_position,
            strict: span.strict,
        }
        .into()
    }

    fn has_inappropriate_newlines(&self, elements: &ReflowSequenceType, strict: bool) -> bool {
        let n_prev_newlines = elements[self.prev.newline_pt_idx as usize].num_newlines();
        let n_next_newlines = elements[self.next.newline_pt_idx as usize].num_newlines();

        let newlines_on_neither_side = n_prev_newlines + n_next_newlines == 0;
        let newlines_on_both_sides = n_prev_newlines > 0 && n_next_newlines > 0;

        (newlines_on_neither_side && !strict) || newlines_on_both_sides
    }

    fn pretty_target_name(&self) -> String {
        format!("{} {}", self.target.get_type().as_str(), self.target.raw())
    }
}

pub fn identify_rebreak_spans(
    element_buffer: &ReflowSequenceType,
    root_segment: &ErasedSegment,
) -> Vec<RebreakSpan> {
    let mut spans = Vec::new();

    for (idx, elem) in element_buffer
        .iter()
        .enumerate()
        .take(element_buffer.len() - 2)
        .skip(2)
    {
        let ReflowElement::Block(block) = elem else {
            continue;
        };

        if let Some(original_line_position) = block.line_position() {
            let line_position = original_line_position.first().unwrap();
            spans.push(RebreakSpan {
                target: elem.segments().first().cloned().unwrap(),
                start_idx: idx,
                end_idx: idx,
                line_position: *line_position,
                strict: original_line_position.last() == Some(&LinePosition::Strict),
            });
        }

        for key in block.line_position_configs().keys() {
            let mut final_idx = None;
            if block.depth_info().stack_positions[key].idx != 0 {
                continue;
            }

            for end_idx in idx..element_buffer.len() {
                let end_elem = &element_buffer[end_idx];
                let ReflowElement::Block(end_block) = end_elem else {
                    continue;
                };

                if !end_block.depth_info().stack_positions.contains_key(key) {
                    final_idx = (end_idx - 2).into();
                } else if matches!(
                    end_block.depth_info().stack_positions[key].type_,
                    Some(StackPositionType::End) | Some(StackPositionType::Solo)
                ) {
                    final_idx = end_idx.into();
                }

                if let Some(final_idx) = final_idx {
                    let target_depth = block
                        .depth_info()
                        .stack_hashes
                        .iter()
                        .position(|it| it == key)
                        .unwrap();
                    let target = root_segment.path_to(&element_buffer[idx].segments()[0])
                        [target_depth]
                        .segment
                        .clone();

                    let line_position_configs = block.line_position_configs()[key]
                        .split(':')
                        .next()
                        .unwrap();
                    let line_position = LinePosition::from_str(line_position_configs).unwrap();

                    spans.push(RebreakSpan {
                        target,
                        start_idx: idx,
                        end_idx: final_idx,
                        line_position,
                        strict: block.line_position_configs()[key].ends_with("strict"),
                    });

                    break;
                }
            }
        }
    }

    spans
}

pub fn rebreak_sequence(
    tables: &Tables,
    elements: ReflowSequenceType,
    root_segment: &ErasedSegment,
) -> (ReflowSequenceType, Vec<LintResult>) {
    let mut lint_results = Vec::new();
    let mut fixes = Vec::new();
    let mut elem_buff = elements.clone();

    // Given a sequence we should identify the objects which
    // make sense to rebreak. That includes any raws with config,
    // but also and parent segments which have config and we can
    // find both ends for. Given those spans, we then need to find
    // the points either side of them and then the blocks either
    // side to respace them at the same time.

    // 1. First find appropriate spans.
    let spans = identify_rebreak_spans(&elem_buff, root_segment);

    let mut locations = Vec::new();
    for span in spans {
        if let Some(loc) = RebreakLocation::from_span(span, &elements) {
            locations.push(loc);
        }
    }

    // Handle each span:
    for loc in locations {
        if loc.has_inappropriate_newlines(&elements, loc.strict) {
            continue;
        }

        // if loc.has_templated_newline(elem_buff) {
        //     continue;
        // }

        // Points and blocks either side are just offsets from the indices.
        let prev_point = elem_buff[loc.prev.adj_pt_idx as usize]
            .as_point()
            .unwrap()
            .clone();
        let next_point = elem_buff[loc.next.adj_pt_idx as usize]
            .as_point()
            .unwrap()
            .clone();

        // So we know we have a preference, is it ok?
        let new_results = if loc.line_position == LinePosition::Leading {
            if elem_buff[loc.prev.newline_pt_idx as usize].num_newlines() != 0 {
                // We're good. It's already leading.
                continue;
            }

            // Generate the text for any issues.
            let pretty_name = loc.pretty_target_name();
            let _desc = if loc.strict {
                format!(
                    "{} should always start a new line.",
                    capitalize(&pretty_name)
                )
            } else {
                format!(
                    "Found trailing {}. Expected only leading near line breaks.",
                    pretty_name
                )
            };

            if loc.next.adj_pt_idx == loc.next.pre_code_pt_idx
                && elem_buff[loc.next.newline_pt_idx as usize].num_newlines() == 1
            {
                // Simple case. No comments.
                // Strip newlines from the next point.
                // Apply the indent to the previous point.

                let desired_indent = next_point.get_indent().unwrap_or_default();

                let (new_results, prev_point) = prev_point.indent_to(
                    tables,
                    &desired_indent,
                    None,
                    loc.target.clone().into(),
                    None,
                    None,
                );

                let (new_results, next_point) = next_point.respace_point(
                    tables,
                    elem_buff[loc.next.adj_pt_idx as usize - 1].as_block(),
                    elem_buff[loc.next.adj_pt_idx as usize + 1].as_block(),
                    root_segment,
                    new_results,
                    true,
                    "before",
                );

                // Update the points in the buffer
                elem_buff[loc.prev.adj_pt_idx as usize] = prev_point.into();
                elem_buff[loc.next.adj_pt_idx as usize] = next_point.into();

                new_results
            } else {
                fixes.push(LintFix::delete(loc.target.clone()));
                for seg in elem_buff[loc.prev.adj_pt_idx as usize].segments() {
                    fixes.push(LintFix::delete(seg.clone()));
                }

                let (new_results, new_point) = ReflowPoint::new(Vec::new()).respace_point(
                    tables,
                    elem_buff[(loc.next.adj_pt_idx - 1) as usize].as_block(),
                    elem_buff[(loc.next.pre_code_pt_idx + 1) as usize].as_block(),
                    root_segment,
                    Vec::new(),
                    false,
                    "after",
                );

                let mut create_anchor = None;
                for i in 0..loc.next.pre_code_pt_idx {
                    let idx = loc.next.pre_code_pt_idx - i;
                    if let Some(elem) = elem_buff.get(idx as usize) {
                        if let Some(segments) = elem.segments().last() {
                            create_anchor = Some(segments.clone());
                            break;
                        }
                    }
                }

                if create_anchor.is_none() {
                    panic!("Could not find anchor for creation.");
                }

                fixes.push(LintFix::create_after(
                    create_anchor.unwrap(),
                    vec![loc.target.clone()],
                    None,
                ));

                rearrange_and_insert(&mut elem_buff, &loc, new_point);

                new_results
            }
        } else if loc.line_position == LinePosition::Trailing {
            if elem_buff[loc.next.newline_pt_idx as usize].num_newlines() != 0 {
                continue;
            }

            let pretty_name = loc.pretty_target_name();
            let _desc = if loc.strict {
                format!(
                    "{} should always be at the end of a line.",
                    capitalize(&pretty_name)
                )
            } else {
                format!(
                    "Found leading {}. Expected only trailing near line breaks.",
                    pretty_name
                )
            };

            if loc.prev.adj_pt_idx == loc.prev.pre_code_pt_idx
                && elem_buff[loc.prev.newline_pt_idx as usize].num_newlines() == 1
            {
                let (new_results, next_point) = next_point.indent_to(
                    tables,
                    prev_point.get_indent().as_deref().unwrap_or_default(),
                    Some(loc.target.clone()),
                    None,
                    None,
                    None,
                );

                let (new_results, prev_point) = prev_point.respace_point(
                    tables,
                    elem_buff[loc.prev.adj_pt_idx as usize - 1].as_block(),
                    elem_buff[loc.prev.adj_pt_idx as usize + 1].as_block(),
                    root_segment,
                    new_results,
                    true,
                    "before",
                );

                // Update the points in the buffer
                elem_buff[loc.prev.adj_pt_idx as usize] = prev_point.into();
                elem_buff[loc.next.adj_pt_idx as usize] = next_point.into();

                new_results
            } else {
                fixes.push(LintFix::delete(loc.target.clone()));
                for seg in elem_buff[loc.next.adj_pt_idx as usize].segments() {
                    fixes.push(LintFix::delete(seg.clone()));
                }

                let (new_results, new_point) = ReflowPoint::new(Vec::new()).respace_point(
                    tables,
                    elem_buff[(loc.prev.pre_code_pt_idx - 1) as usize].as_block(),
                    elem_buff[(loc.prev.adj_pt_idx + 1) as usize].as_block(),
                    root_segment,
                    Vec::new(),
                    false,
                    "before",
                );

                fixes.push(LintFix::create_before(
                    elem_buff[loc.prev.pre_code_pt_idx as usize].segments()[0].clone(),
                    vec![loc.target.clone()],
                ));

                reorder_and_insert(&mut elem_buff, &loc, new_point);

                new_results
            }
        } else if loc.line_position == LinePosition::Alone {
            let mut new_results = Vec::new();

            if elem_buff[loc.next.newline_pt_idx as usize].num_newlines() == 0 {
                let (results, next_point) = next_point.indent_to(
                    tables,
                    &deduce_line_indent(
                        loc.target.get_raw_segments().last().unwrap(),
                        root_segment,
                    ),
                    loc.target.clone().into(),
                    None,
                    None,
                    None,
                );

                new_results = results;
                elem_buff[loc.next.adj_pt_idx as usize] = next_point.into();
            }

            if elem_buff[loc.prev.adj_pt_idx as usize].num_newlines() == 0 {
                let (results, prev_point) = prev_point.indent_to(
                    tables,
                    &deduce_line_indent(
                        loc.target.get_raw_segments().first().unwrap(),
                        root_segment,
                    ),
                    None,
                    loc.target.clone().into(),
                    None,
                    None,
                );

                new_results = results;
                elem_buff[loc.prev.adj_pt_idx as usize] = prev_point.into();
            }

            new_results
        } else {
            unimplemented!(
                "Unexpected line_position config: {}",
                loc.line_position.as_ref()
            )
        };

        let fixes = fixes_from_results(new_results.into_iter())
            .chain(std::mem::take(&mut fixes))
            .collect();
        lint_results.push(LintResult::new(
            loc.target.clone().into(),
            fixes,
            None,
            None,
        ));
    }

    (elem_buff, lint_results)
}

fn rearrange_and_insert(
    elem_buff: &mut Vec<ReflowElement>,
    loc: &RebreakLocation,
    new_point: ReflowPoint,
) {
    let mut new_buff = Vec::with_capacity(elem_buff.len() + 1);

    // First segment: up to loc.prev.adj_pt_idx (exclusive)
    new_buff.extend_from_slice(&elem_buff[..loc.prev.adj_pt_idx as usize]);

    // Second segment: loc.next.adj_pt_idx to loc.next.pre_code_pt_idx (inclusive)
    new_buff.extend_from_slice(
        &elem_buff[loc.next.adj_pt_idx as usize..=loc.next.pre_code_pt_idx as usize],
    );

    // Third segment: loc.prev.adj_pt_idx + 1 to loc.next.adj_pt_idx (exclusive, the
    // target)
    if loc.prev.adj_pt_idx + 1 < loc.next.adj_pt_idx {
        new_buff.extend_from_slice(
            &elem_buff[loc.prev.adj_pt_idx as usize + 1..loc.next.adj_pt_idx as usize],
        );
    }

    // Insert new_point here
    new_buff.push(new_point.into());

    // Last segment: after loc.next.pre_code_pt_idx
    if loc.next.pre_code_pt_idx as usize + 1 < elem_buff.len() {
        new_buff.extend_from_slice(&elem_buff[loc.next.pre_code_pt_idx as usize + 1..]);
    }

    // Replace old buffer with the new one
    *elem_buff = new_buff;
}

fn reorder_and_insert(
    elem_buff: &mut Vec<ReflowElement>,
    loc: &RebreakLocation,
    new_point: ReflowPoint,
) {
    let mut new_buff = Vec::with_capacity(elem_buff.len() + 1);

    // First segment: up to loc.prev.pre_code_pt_idx (exclusive)
    new_buff.extend_from_slice(&elem_buff[..loc.prev.pre_code_pt_idx as usize]);

    // Insert new_point here
    new_buff.push(new_point.into());

    // Second segment: loc.prev.adj_pt_idx + 1 to loc.next.adj_pt_idx (exclusive,
    // the target)
    if loc.prev.adj_pt_idx + 1 < loc.next.adj_pt_idx {
        new_buff.extend_from_slice(
            &elem_buff[loc.prev.adj_pt_idx as usize + 1..loc.next.adj_pt_idx as usize],
        );
    }

    // Third segment: loc.prev.pre_code_pt_idx to loc.prev.adj_pt_idx + 1
    // (inclusive)
    new_buff.extend_from_slice(
        &elem_buff[loc.prev.pre_code_pt_idx as usize..=loc.prev.adj_pt_idx as usize],
    );

    // Last segment: after loc.next.adj_pt_idx
    if loc.next.adj_pt_idx as usize + 1 < elem_buff.len() {
        new_buff.extend_from_slice(&elem_buff[loc.next.adj_pt_idx as usize + 1..]);
    }

    // Replace old buffer with the new one
    *elem_buff = new_buff;
}

#[cfg(test)]
mod tests {
    use sqruff_lib::core::test_functions::parse_ansi_string;
    use sqruff_lib_core::helpers::enter_panic;
    use sqruff_lib_core::parser::segments::base::Tables;

    use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

    #[test]
    fn test_reflow_sequence_rebreak_root() {
        let cases = [
            // Trivial Case
            ("select 1", "select 1"),
            // These rely on the default config being for leading operators
            ("select 1\n+2", "select 1\n+2"),
            ("select 1+\n2", "select 1\n+ 2"), // NOTE: Implicit respace.
            ("select\n  1 +\n  2", "select\n  1\n  + 2"),
            (
                "select\n  1 +\n  -- comment\n  2",
                "select\n  1\n  -- comment\n  + 2",
            ),
            // These rely on the default config being for trailing commas
            ("select a,b", "select a,b"),
            ("select a\n,b", "select a,\nb"),
            ("select\n  a\n  , b", "select\n  a,\n  b"),
            ("select\n    a\n    , b", "select\n    a,\n    b"),
            ("select\n  a\n    , b", "select\n  a,\n    b"),
            (
                "select\n  a\n  -- comment\n  , b",
                "select\n  a,\n  -- comment\n  b",
            ),
        ];

        let tables = Tables::default();
        for (raw_sql_in, raw_sql_out) in cases {
            let _panic = enter_panic(format!("{raw_sql_in:?}"));

            let root = parse_ansi_string(raw_sql_in);
            let config = <_>::default();
            let seq = ReflowSequence::from_root(root, &config);
            let new_seq = seq.rebreak(&tables);

            assert_eq!(new_seq.raw(), raw_sql_out);
        }
    }

    #[test]
    fn test_reflow_sequence_rebreak_target() {
        let cases = [
            ("select 1+\n(2+3)", 4, "1+\n(", "1\n+ ("),
            ("select a,\n(b+c)", 4, "a,\n(", "a,\n("),
            ("select a\n  , (b+c)", 6, "a\n  , (", "a,\n  ("),
            // Here we don't have enough context to rebreak it so
            // it should be left unaltered.
            ("select a,\n(b+c)", 6, ",\n(b", ",\n(b"),
            // This intentionally targets an incomplete span.
            ("select a<=b", 4, "a<=", "a<="),
        ];

        let tables = Tables::default();
        for (raw_sql_in, target_idx, seq_sql_in, seq_sql_out) in cases {
            let root = parse_ansi_string(raw_sql_in);
            let target = &root.get_raw_segments()[target_idx];
            let config = <_>::default();
            let seq = ReflowSequence::from_around_target(target, root, TargetSide::Both, &config);

            assert_eq!(seq.raw(), seq_sql_in);

            let new_seq = seq.rebreak(&tables);
            assert_eq!(new_seq.raw(), seq_sql_out);
        }
    }
}
