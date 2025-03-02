use itertools::{Itertools, enumerate};
use rustc_hash::FxHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::edit_type::EditType;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::markers::PositionMarker;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};

use super::elements::ReflowBlock;
use crate::core::rules::base::LintResult;
use crate::utils::reflow::config::Spacing;
use crate::utils::reflow::helpers::pretty_segment_name;

fn unpack_constraint(constraint: Spacing, strip_newlines: bool) -> (Spacing, bool) {
    match constraint {
        Spacing::TouchInline => (Spacing::Touch, true),
        Spacing::SingleInline => (Spacing::Single, true),
        _ => (constraint, strip_newlines),
    }
}

pub fn determine_constraints(
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    strip_newlines: bool,
) -> (Spacing, Spacing, bool) {
    // Start with the defaults
    let (mut pre_constraint, strip_newlines) = unpack_constraint(
        if let Some(prev_block) = prev_block {
            prev_block.spacing_after()
        } else {
            Spacing::Single
        },
        strip_newlines,
    );

    let (mut post_constraint, mut strip_newlines) = unpack_constraint(
        if let Some(next_block) = next_block {
            next_block.spacing_before()
        } else {
            Spacing::Single
        },
        strip_newlines,
    );

    let mut within_spacing = None;
    let mut idx = None;

    if let Some((prev_block, next_block)) = prev_block.zip(next_block) {
        let common = prev_block.depth_info().common_with(next_block.depth_info());
        let last_common = common.last().unwrap();
        idx = prev_block
            .depth_info()
            .stack_hashes
            .iter()
            .position(|p| p == last_common)
            .unwrap()
            .into();

        let within_constraint = prev_block.stack_spacing_configs().get(last_common);
        if let Some(within_constraint) = within_constraint {
            let (within_spacing_inner, strip_newlines_inner) =
                unpack_constraint(*within_constraint, strip_newlines);

            within_spacing = Some(within_spacing_inner);
            strip_newlines = strip_newlines_inner;
        }
    }

    match within_spacing {
        Some(Spacing::Touch) => {
            if pre_constraint != Spacing::Any {
                pre_constraint = Spacing::Touch;
            }
            if post_constraint != Spacing::Any {
                post_constraint = Spacing::Touch;
            }
        }
        Some(Spacing::Any) => {
            pre_constraint = Spacing::Any;
            post_constraint = Spacing::Any;
        }
        Some(Spacing::Single) => {}
        Some(spacing) => {
            panic!(
                "Unexpected within constraint: {:?} for {:?}",
                spacing,
                prev_block.unwrap().depth_info().stack_class_types[idx.unwrap()]
            );
        }
        _ => {}
    }

    (pre_constraint, post_constraint, strip_newlines)
}

pub fn process_spacing(
    segment_buffer: &[ErasedSegment],
    strip_newlines: bool,
) -> (Vec<ErasedSegment>, Option<ErasedSegment>, Vec<LintResult>) {
    let mut removal_buffer = Vec::new();
    let mut result_buffer = Vec::new();
    let mut last_whitespace = Vec::new();

    // Loop through the existing segments looking for spacing.
    for seg in segment_buffer {
        // If it's whitespace, store it.
        if seg.is_type(SyntaxKind::Whitespace) {
            last_whitespace.push(seg.clone());
        }
        // If it's a newline, react accordingly.
        // NOTE: This should only trigger on literal newlines.
        else if matches!(seg.get_type(), SyntaxKind::Newline | SyntaxKind::EndOfFile) {
            if seg
                .get_position_marker()
                .is_some_and(|pos_marker| !pos_marker.is_literal())
            {
                last_whitespace.clear();
                continue;
            }

            if strip_newlines && seg.is_type(SyntaxKind::Newline) {
                removal_buffer.push(seg.clone());
                result_buffer.push(LintResult::new(
                    seg.clone().into(),
                    vec![LintFix::delete(seg.clone())],
                    Some("Unexpected line break.".into()),
                    None,
                ));
                continue;
            }

            if !last_whitespace.is_empty() {
                for ws in last_whitespace.drain(..) {
                    removal_buffer.push(ws.clone());
                    result_buffer.push(LintResult::new(
                        ws.clone().into(),
                        vec![LintFix::delete(ws)],
                        Some("Unnecessary trailing whitespace.".into()),
                        None,
                    ))
                }
            }
        }
    }

    if last_whitespace.len() >= 2 {
        let seg = segment_buffer.last().unwrap();

        for ws in last_whitespace.iter().skip(1).cloned() {
            removal_buffer.push(ws.clone());
            result_buffer.push(LintResult::new(
                seg.clone().into(),
                vec![LintFix::delete(ws)],
                "Unnecessary trailing whitespace.".to_owned().into(),
                None,
            ));
        }
    }

    // Turn the removal buffer updated segment buffer, last whitespace and
    // associated fixes.

    let filtered_segment_buffer = segment_buffer
        .iter()
        .filter(|s| !removal_buffer.contains(s))
        .cloned()
        .collect_vec();

    (
        filtered_segment_buffer,
        last_whitespace.first().cloned(),
        result_buffer,
    )
}

fn determine_aligned_inline_spacing(
    root_segment: &ErasedSegment,
    whitespace_seg: &ErasedSegment,
    next_seg: &ErasedSegment,
    mut next_pos: PositionMarker,
    segment_type: SyntaxKind,
    align_within: Option<SyntaxKind>,
    align_scope: Option<SyntaxKind>,
) -> String {
    // Find the level of segment that we're aligning.
    let mut parent_segment = None;

    // Edge case: if next_seg has no position, we should use the position
    // of the whitespace for searching.
    if let Some(align_within) = align_within {
        for ps in root_segment
            .path_to(if next_seg.get_position_marker().is_some() {
                next_seg
            } else {
                whitespace_seg
            })
            .iter()
            .rev()
        {
            if ps.segment.is_type(align_within) {
                parent_segment = Some(ps.segment.clone());
            }
            if let Some(align_scope) = align_scope {
                if ps.segment.is_type(align_scope) {
                    break;
                }
            }
        }
    }

    if parent_segment.is_none() {
        return " ".to_string();
    }

    let parent_segment = parent_segment.unwrap();

    // We've got a parent. Find some siblings.
    let mut siblings = Vec::new();
    for sibling in parent_segment.recursive_crawl(
        &SyntaxSet::single(segment_type),
        true,
        &SyntaxSet::EMPTY,
        true,
    ) {
        // Purge any siblings with a boundary between them
        if align_scope.is_none()
            || !parent_segment
                .path_to(&sibling)
                .iter()
                .any(|ps| ps.segment.is_type(align_scope.unwrap()))
        {
            siblings.push(sibling);
        }
    }

    // If the segment we're aligning, has position. Use that position.
    // If it doesn't, then use the provided one. We can't do sibling analysis
    // without it.
    if let Some(pos_marker) = next_seg.get_position_marker() {
        next_pos = pos_marker.clone();
    }

    // Purge any siblings which are either self, or on the same line but after it.
    let mut earliest_siblings: FxHashMap<usize, usize> = FxHashMap::default();
    siblings.retain(|sibling| {
        let pos_marker = sibling.get_position_marker().unwrap();
        let best_seen = earliest_siblings.get(&pos_marker.working_line_no).copied();
        if let Some(best_seen) = best_seen {
            if pos_marker.working_line_pos > best_seen {
                return false;
            }
        }
        earliest_siblings.insert(pos_marker.working_line_no, pos_marker.working_line_pos);

        if pos_marker.working_line_no == next_pos.working_line_no
            && pos_marker.working_line_pos != next_pos.working_line_pos
        {
            return false;
        }
        true
    });

    // If there's only one sibling, we have nothing to compare to. Default to a
    // single space.
    if siblings.len() <= 1 {
        return " ".to_string();
    }

    let mut last_code: Option<ErasedSegment> = None;
    let mut max_desired_line_pos = 0;

    for seg in parent_segment.get_raw_segments() {
        for sibling in &siblings {
            if let (Some(seg_pos), Some(sibling_pos)) =
                (&seg.get_position_marker(), &sibling.get_position_marker())
            {
                if seg_pos.working_loc() == sibling_pos.working_loc() {
                    if let Some(last_code) = &last_code {
                        let loc = last_code
                            .get_position_marker()
                            .unwrap()
                            .working_loc_after(last_code.raw());

                        if loc.1 > max_desired_line_pos {
                            max_desired_line_pos = loc.1;
                        }
                    }
                }
            }
        }

        if seg.is_code() {
            last_code = Some(seg.clone());
        }
    }

    " ".repeat(
        1 + max_desired_line_pos
            - whitespace_seg
                .get_position_marker()
                .as_ref()
                .unwrap()
                .working_line_pos,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn handle_respace_inline_with_space(
    tables: &Tables,
    pre_constraint: Spacing,
    post_constraint: Spacing,
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    root_segment: &ErasedSegment,
    mut segment_buffer: Vec<ErasedSegment>,
    last_whitespace: ErasedSegment,
) -> (Vec<ErasedSegment>, Vec<LintResult>) {
    // Get some indices so that we can reference around them
    let ws_idx = segment_buffer
        .iter()
        .position(|it| it == &last_whitespace)
        .unwrap();

    if pre_constraint == Spacing::Any || post_constraint == Spacing::Any {
        return (segment_buffer, vec![]);
    }

    if [pre_constraint, post_constraint].contains(&Spacing::Touch) {
        segment_buffer.remove(ws_idx);

        let description = if let Some(next_block) = next_block {
            format!(
                "Unexpected whitespace before {}.",
                pretty_segment_name(next_block.segment())
            )
        } else {
            "Unexpected whitespace".to_string()
        };

        let lint_result = LintResult::new(
            last_whitespace.clone().into(),
            vec![LintFix::delete(last_whitespace)],
            Some(description),
            None,
        );

        // Return the segment buffer and the lint result
        return (segment_buffer, vec![lint_result]);
    }

    // Handle left alignment & singles
    if (matches!(post_constraint, Spacing::Align { .. }) && next_block.is_some())
        || pre_constraint == Spacing::Single && post_constraint == Spacing::Single
    {
        let (desc, desired_space) = match (post_constraint, next_block) {
            (
                Spacing::Align {
                    seg_type,
                    within,
                    scope,
                },
                Some(next_block),
            ) => {
                let next_pos = if let Some(pos_marker) = next_block.segment().get_position_marker()
                {
                    Some(pos_marker.clone())
                } else if let Some(pos_marker) = last_whitespace.get_position_marker() {
                    Some(pos_marker.end_point_marker())
                } else if let Some(prev_block) = prev_block {
                    prev_block
                        .segment()
                        .get_position_marker()
                        .map(|pos_marker| pos_marker.end_point_marker())
                } else {
                    None
                };

                if let Some(next_pos) = next_pos {
                    let desired_space = determine_aligned_inline_spacing(
                        root_segment,
                        &last_whitespace,
                        next_block.segment(),
                        next_pos,
                        seg_type,
                        within,
                        scope,
                    );
                    ("Item misaligned".to_string(), desired_space)
                } else {
                    ("Item misaligned".to_string(), " ".to_string())
                }
            }
            _ => {
                let desc = if let Some(next_block) = next_block {
                    format!(
                        "Expected only single space before {:?}. Found {:?}.",
                        next_block.segment().raw(),
                        last_whitespace.raw()
                    )
                } else {
                    format!(
                        "Expected only single space. Found {:?}.",
                        last_whitespace.raw()
                    )
                };
                let desired_space = " ".to_string();
                (desc, desired_space)
            }
        };

        let mut new_results = Vec::new();
        if last_whitespace.raw().as_str() != desired_space {
            let new_seg = last_whitespace.edit(tables.next_id(), desired_space.into(), None);

            new_results.push(LintResult::new(
                last_whitespace.clone().into(),
                vec![LintFix::replace(
                    last_whitespace,
                    vec![new_seg.clone()],
                    None,
                )],
                Some(desc),
                None,
            ));
            segment_buffer[ws_idx] = new_seg;
        }

        return (segment_buffer, new_results);
    }

    unimplemented!("Unexpected Constraints: {pre_constraint:?}, {post_constraint:?}");
}

#[allow(clippy::too_many_arguments)]
pub fn handle_respace_inline_without_space(
    tables: &Tables,
    pre_constraint: Spacing,
    post_constraint: Spacing,
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    mut segment_buffer: Vec<ErasedSegment>,
    mut existing_results: Vec<LintResult>,
    anchor_on: &str,
) -> (Vec<ErasedSegment>, Vec<LintResult>, bool) {
    let constraints = [Spacing::Touch, Spacing::Any];

    if constraints.contains(&pre_constraint) || constraints.contains(&post_constraint) {
        return (segment_buffer, existing_results, false);
    }

    let added_whitespace = SegmentBuilder::whitespace(tables.next_id(), " ");

    // Add it to the buffer first (the easy bit). The hard bit is to then determine
    // how to generate the appropriate LintFix objects.
    segment_buffer.push(added_whitespace.clone());

    // So special handling here. If segments either side already exist then we don't
    // care which we anchor on but if one is already an insertion (as shown by a
    // lack) of pos_marker, then we should piggyback on that pre-existing fix.
    let mut existing_fix = None;
    let mut insertion = None;

    if let Some(block) = prev_block {
        if block.segment().get_position_marker().is_none() {
            existing_fix = Some("after");
            insertion = Some(block.segment().clone());
        }
    } else if let Some(block) = next_block {
        if block.segment().get_position_marker().is_none() {
            existing_fix = Some("before");
            insertion = Some(block.segment().clone());
        }
    }

    if let Some(existing_fix) = existing_fix {
        let mut res_found = None;
        let mut fix_found = None;

        'outer: for (result_idx, res) in enumerate(&existing_results) {
            for (fix_idx, fix) in enumerate(&res.fixes) {
                if fix
                    .edit
                    .iter()
                    .any(|e| e.id() == insertion.as_ref().unwrap().id())
                {
                    res_found = Some(result_idx);
                    fix_found = Some(fix_idx);
                    break 'outer;
                }
            }
        }

        let res = res_found.unwrap();
        let fix = fix_found.unwrap();

        let fix = &mut existing_results[res].fixes[fix];

        if existing_fix == "before" {
            unimplemented!()
        } else if existing_fix == "after" {
            fix.edit.push(added_whitespace);
        }

        return (segment_buffer, existing_results, true);
    }

    let desc = if let Some((prev_block, next_block)) = prev_block.zip(next_block) {
        format!(
            "Expected single whitespace between {:?} and {:?}.",
            prev_block.segment().raw(),
            next_block.segment().raw()
        )
    } else {
        "Expected single whitespace.".to_owned()
    };

    let new_result = if prev_block.is_some() && anchor_on != "after" {
        let prev_block = prev_block.unwrap();
        let anchor = if let Some(block) = next_block {
            // If next_block is Some, get the first segment
            block.segment().clone()
        } else {
            prev_block.segment().clone()
        };

        LintResult::new(
            anchor.into(),
            vec![LintFix {
                edit_type: EditType::CreateAfter,
                anchor: prev_block.segment().clone(),
                edit: vec![added_whitespace],
                source: vec![],
            }],
            desc.into(),
            None,
        )
    } else if let Some(next_block) = next_block {
        LintResult::new(
            next_block.segment().clone().into(),
            vec![LintFix::create_before(
                next_block.segment().clone(),
                vec![SegmentBuilder::whitespace(tables.next_id(), " ")],
            )],
            Some(desc),
            None,
        )
    } else {
        unimplemented!("Not set up to handle a missing _after_ and _before_.")
    };

    existing_results.push(new_result);
    (segment_buffer, existing_results, false)
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use smol_str::ToSmolStr;
    use sqruff_lib::core::test_functions::parse_ansi_string;
    use sqruff_lib_core::edit_type::EditType;
    use sqruff_lib_core::helpers::enter_panic;

    use crate::utils::reflow::helpers::fixes_from_results;
    use crate::utils::reflow::respace::Tables;
    use crate::utils::reflow::sequence::{Filter, ReflowSequence};

    #[test]
    fn test_reflow_sequence_respace() {
        let cases = [
            // Basic cases
            ("select 1+2", (false, Filter::All), "select 1 + 2"),
            (
                "select    1   +   2    ",
                (false, Filter::All),
                "select 1 + 2",
            ),
            // Check newline handling
            (
                "select\n    1   +   2",
                (false, Filter::All),
                "select\n    1 + 2",
            ),
            ("select\n    1   +   2", (true, Filter::All), "select 1 + 2"),
            // Check filtering
            (
                "select  \n  1   +   2 \n ",
                (false, Filter::All),
                "select\n  1 + 2\n",
            ),
            (
                "select  \n  1   +   2 \n ",
                (false, Filter::Inline),
                "select  \n  1 + 2 \n ",
            ),
            (
                "select  \n  1   +   2 \n ",
                (false, Filter::Newline),
                "select\n  1   +   2\n",
            ),
        ];

        let tables = Tables::default();
        for (raw_sql_in, (strip_newlines, filter), raw_sql_out) in cases {
            let root = parse_ansi_string(raw_sql_in);
            let config = <_>::default();
            let seq = ReflowSequence::from_root(root, &config);

            let new_seq = seq.respace(&tables, strip_newlines, filter);
            assert_eq!(new_seq.raw(), raw_sql_out);
        }
    }

    #[test]
    fn test_reflow_point_respace_point() {
        let cases = [
            // Basic cases
            (
                "select    1",
                1,
                false,
                " ",
                vec![(EditType::Replace, "    ".into())],
            ),
            (
                "select 1+2",
                3,
                false,
                " ",
                vec![(EditType::CreateAfter, "1".into())],
            ),
            ("select (1+2)", 3, false, "", vec![]),
            (
                "select (  1+2)",
                3,
                false,
                "",
                vec![(EditType::Delete, "  ".into())],
            ),
            // Newline handling
            ("select\n1", 1, false, "\n", vec![]),
            ("select\n  1", 1, false, "\n  ", vec![]),
            (
                "select  \n  1",
                1,
                false,
                "\n  ",
                vec![(EditType::Delete, "  ".into())],
            ),
            (
                "select  \n 1",
                1,
                true,
                " ",
                vec![
                    (EditType::Delete, "\n".into()),
                    (EditType::Delete, " ".into()),
                    (EditType::Replace, "  ".into()),
                ],
            ),
            (
                "select ( \n  1)",
                3,
                true,
                "",
                vec![
                    (EditType::Delete, "\n".into()),
                    (EditType::Delete, "  ".into()),
                    (EditType::Delete, " ".into()),
                ],
            ),
        ];

        let tables = Tables::default();
        for (raw_sql_in, point_idx, strip_newlines, raw_point_sql_out, fixes_out) in cases {
            let _panic = enter_panic(format!("{raw_sql_in:?}"));

            let root = parse_ansi_string(raw_sql_in);
            let config = <_>::default();
            let seq = ReflowSequence::from_root(root.clone(), &config);
            let pnt = seq.elements()[point_idx].as_point().unwrap();

            let (results, new_pnt) = pnt.respace_point(
                &tables,
                seq.elements()[point_idx - 1].as_block(),
                seq.elements()[point_idx + 1].as_block(),
                &root,
                Vec::new(),
                strip_newlines,
                "before",
            );

            assert_eq!(new_pnt.raw(), raw_point_sql_out);

            let fixes = fixes_from_results(results.into_iter())
                .map(|fix| (fix.edit_type, fix.anchor.raw().to_smolstr()))
                .collect_vec();

            assert_eq!(fixes, fixes_out);
        }
    }
}
