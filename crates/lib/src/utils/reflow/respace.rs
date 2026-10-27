use std::collections::BTreeMap;

use itertools::{Itertools, enumerate};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::markers::PositionMarker;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};

use super::elements::ReflowBlock;
use crate::core::rules::LintResult;
use crate::utils::reflow::config::{AlignmentCoordinateSpace, Spacing};
use crate::utils::reflow::helpers::pretty_segment_name;
use crate::utils::reflow::reindent::IndentUnit;

// The inline comment marker. Every dialect SQLFluff ships lexes "--" this way,
// so no dialect lookup is needed to know that building one changes meaning.
const INLINE_COMMENT_MARKER: &str = "--";

/// Would removing the whitespace between these blocks start a comment?
///
/// Spacing configs such as `touch` are resolved against segments, but the fix
/// they produce is applied to the raw file. Deleting the whitespace in
/// `1 * - - 5` glues the two sign indicators into `--`, which the lexer then
/// reads as an inline comment - so the rest of the line disappears without the
/// file ever becoming unparsable.
fn would_start_comment(prev_block: Option<&ReflowBlock>, next_block: Option<&ReflowBlock>) -> bool {
    let (Some(prev_block), Some(next_block)) = (prev_block, next_block) else {
        // A point at the very start or end of the file has nothing to fuse with.
        return false;
    };
    let prev_raw = prev_block.segment().raw();
    let next_raw = next_block.segment().raw();
    let (Some(prev_last), Some(next_first)) = (prev_raw.chars().last(), next_raw.chars().next())
    else {
        // Zero length blocks (e.g. templated placeholders) have no characters
        // to fuse, so they can't build a marker on their own.
        return false;
    };
    let mut marker = String::with_capacity(2);
    marker.push(prev_last);
    marker.push(next_first);
    marker == INLINE_COMMENT_MARKER
}

fn construct_alignment_whitespace(width: usize, indent_unit: IndentUnit) -> String {
    match indent_unit {
        IndentUnit::Tab => "\t".repeat(width),
        IndentUnit::Space(_) => " ".repeat(width),
    }
}

fn position_line(pos_marker: &PositionMarker, use_source: bool) -> usize {
    if use_source {
        pos_marker.line_no()
    } else {
        pos_marker.working_line_no
    }
}

fn position_column(
    pos_marker: &PositionMarker,
    use_source: bool,
    tab_space_size: usize,
    indent_unit: IndentUnit,
) -> usize {
    if use_source {
        pos_marker.line_pos()
    } else if indent_unit == IndentUnit::Tab {
        pos_marker.working_visual_column(tab_space_size)
    } else {
        pos_marker.working_line_pos
    }
}

fn has_templated_content(
    parent_segment: &ErasedSegment,
    siblings: &[ErasedSegment],
    next_seg: &ErasedSegment,
) -> bool {
    next_seg
        .get_position_marker()
        .is_some_and(|pos_marker| !pos_marker.is_literal())
        || siblings.iter().any(|sibling| {
            sibling
                .get_position_marker()
                .is_some_and(|pos_marker| !pos_marker.is_literal())
        })
        || parent_segment.get_raw_segments().iter().any(|segment| {
            segment.is_code()
                && segment
                    .get_position_marker()
                    .is_some_and(|pos_marker| !pos_marker.is_literal())
        })
}

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

        // A newline on either side of a comment must not be stripped: doing
        // so can glue SQL to a comment marker and change the statement.
        if prev_block.segment().is_comment() || next_block.segment().is_comment() {
            strip_newlines = false;
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

    // Prohibit stripping a newline which is the only thing keeping an inline
    // comment marker apart. Both "touch" and "any" leave a point with no
    // whitespace alone, so once the newline goes there is nothing between the
    // blocks and `-` `-` fuses into `--`. Keeping the line break is the only
    // outcome here which preserves the meaning of the file.
    if strip_newlines
        && (matches!(pre_constraint, Spacing::Touch | Spacing::Any)
            || matches!(post_constraint, Spacing::Touch | Spacing::Any))
        && would_start_comment(prev_block, next_block)
    {
        strip_newlines = false;
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
        for ws in last_whitespace.iter().skip(1).cloned() {
            removal_buffer.push(ws.clone());
            result_buffer.push(LintResult::new(
                ws.clone().into(),
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

#[allow(clippy::too_many_arguments)]
fn determine_aligned_inline_spacing(
    root_segment: &ErasedSegment,
    whitespace_seg: &ErasedSegment,
    next_seg: &ErasedSegment,
    mut next_pos: PositionMarker,
    segment_type: SyntaxKind,
    align_within: Option<SyntaxKind>,
    align_scope: Option<SyntaxKind>,
    coordinate_space: Option<AlignmentCoordinateSpace>,
    indent_unit: IndentUnit,
    tab_space_size: usize,
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
            if let Some(align_scope) = align_scope
                && ps.segment.is_type(align_scope)
            {
                break;
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

    let use_source_positions = match coordinate_space {
        Some(AlignmentCoordinateSpace::Source) => true,
        Some(AlignmentCoordinateSpace::Templated) => false,
        None => has_templated_content(&parent_segment, &siblings, next_seg),
    };

    let mut siblings_by_line: BTreeMap<usize, Vec<ErasedSegment>> = BTreeMap::new();
    for sibling in siblings {
        let Some(pos_marker) = sibling.get_position_marker() else {
            continue;
        };
        siblings_by_line
            .entry(position_line(pos_marker, use_source_positions))
            .or_default()
            .push(sibling);
    }

    for line_siblings in siblings_by_line.values_mut() {
        line_siblings.sort_by_key(|sibling| {
            position_column(
                sibling.get_position_marker().unwrap(),
                use_source_positions,
                tab_space_size,
                indent_unit,
            )
        });
    }

    let current_line = position_line(&next_pos, use_source_positions);
    let Some(current_line_segments) = siblings_by_line.get(&current_line) else {
        return " ".to_string();
    };

    let Some(target_index) = current_line_segments.iter().position(|segment| {
        segment.get_position_marker().is_some_and(|pos_marker| {
            position_column(
                pos_marker,
                use_source_positions,
                tab_space_size,
                indent_unit,
            ) == position_column(&next_pos, use_source_positions, tab_space_size, indent_unit)
        })
    }) else {
        return " ".to_string();
    };

    let siblings = siblings_by_line
        .values()
        .filter_map(|segments| segments.get(target_index).cloned())
        .collect_vec();

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
                && seg_pos.working_loc() == sibling_pos.working_loc()
                && let Some(last_code) = &last_code
            {
                let last_code_pos = last_code.get_position_marker().unwrap();
                let loc = if use_source_positions {
                    let end_pos = last_code_pos.end_point_marker();
                    (
                        end_pos.line_no(),
                        position_column(&end_pos, true, tab_space_size, indent_unit),
                    )
                } else {
                    last_code_pos.working_loc_after(last_code.raw())
                };

                // Only siblings whose preceding code is on their own line tell us
                // anything about alignment. When a sibling is the first code on its
                // line, `last_code` is the trailing code of an *earlier* line, and
                // its column is unrelated to where this sibling starts. Counting it
                // inflates the target column and pads every other line to match.
                // See https://github.com/sqlfluff/sqlfluff/issues/8256.
                let sibling_line = position_line(sibling_pos, use_source_positions);
                if loc.0 != sibling_line {
                    continue;
                }

                let desired_line_pos = if indent_unit == IndentUnit::Tab {
                    last_code_pos
                        .end_point_marker()
                        .working_visual_column(tab_space_size)
                } else {
                    loc.1
                };
                max_desired_line_pos = max_desired_line_pos.max(desired_line_pos);
            }
        }

        if seg.is_code() {
            last_code = Some(seg.clone());
        }
    }

    let whitespace_pos = whitespace_seg.get_position_marker().unwrap();
    let whitespace_line = position_line(whitespace_pos, use_source_positions);
    let whitespace_location = if use_source_positions {
        (whitespace_pos.line_no(), whitespace_pos.line_pos())
    } else {
        whitespace_pos.working_loc()
    };

    let mut code_before_whitespace = None;
    for segment in parent_segment.get_raw_segments() {
        let Some(segment_pos) = segment.get_position_marker() else {
            continue;
        };
        let segment_location = if use_source_positions {
            (segment_pos.line_no(), segment_pos.line_pos())
        } else {
            segment_pos.working_loc()
        };
        if segment_location >= whitespace_location {
            break;
        }
        if segment.is_code() && position_line(segment_pos, use_source_positions) == whitespace_line
        {
            code_before_whitespace = Some(segment);
        }
    }

    let current_whitespace_pos = if let Some(code_before_whitespace) = code_before_whitespace {
        let code_pos = code_before_whitespace.get_position_marker().unwrap();
        if use_source_positions || indent_unit == IndentUnit::Tab {
            position_column(
                &code_pos.end_point_marker(),
                use_source_positions,
                tab_space_size,
                indent_unit,
            )
        } else {
            code_pos.working_loc_after(code_before_whitespace.raw()).1
        }
    } else {
        position_column(
            whitespace_pos,
            use_source_positions,
            tab_space_size,
            indent_unit,
        )
    };

    let pad_width = 1 + max_desired_line_pos.saturating_sub(current_whitespace_pos);
    if indent_unit == IndentUnit::Tab {
        let target_column = ((max_desired_line_pos / tab_space_size) + 1) * tab_space_size;
        let mut tab_count = 0;
        let mut position = current_whitespace_pos;
        while position < target_column {
            position = ((position / tab_space_size) + 1) * tab_space_size;
            tab_count += 1;
        }
        construct_alignment_whitespace(tab_count, IndentUnit::Tab)
    } else {
        construct_alignment_whitespace(pad_width, indent_unit)
    }
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
    indent_unit: IndentUnit,
    tab_space_size: usize,
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
        if would_start_comment(prev_block, next_block) {
            // Removing the whitespace would comment out the rest of the line.
            // Leave it alone and don't raise a violation - there is no edit the
            // user could make here which would satisfy the constraint.
            return (segment_buffer, vec![]);
        }

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
                    coordinate_space,
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
                        coordinate_space,
                        indent_unit,
                        tab_space_size,
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

    if let Some(block) = prev_block.filter(|block| block.segment().get_position_marker().is_none())
    {
        existing_fix = Some("after");
        insertion = Some(block.segment().clone());
    } else if let Some(block) =
        next_block.filter(|block| block.segment().get_position_marker().is_none())
    {
        existing_fix = Some("before");
        insertion = Some(block.segment().clone());
    }

    if let Some(existing_fix) = existing_fix {
        let mut res_found = None;
        let mut fix_found = None;

        'outer: for (result_idx, res) in enumerate(&existing_results) {
            for (fix_idx, fix) in enumerate(&res.fixes) {
                let has_insertion = match fix {
                    LintFix::CreateAfter { edit, .. }
                    | LintFix::CreateBefore { edit, .. }
                    | LintFix::Replace { edit, .. } => edit
                        .iter()
                        .any(|e| e.id() == insertion.as_ref().unwrap().id()),
                    LintFix::Delete { .. } => false,
                };
                if has_insertion {
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
            match fix {
                LintFix::CreateAfter { edit, .. }
                | LintFix::CreateBefore { edit, .. }
                | LintFix::Replace { edit, .. } => {
                    edit.insert(0, added_whitespace);
                }
                LintFix::Delete { .. } => unreachable!(),
            }
        } else if existing_fix == "after" {
            match fix {
                LintFix::CreateAfter { edit, .. }
                | LintFix::CreateBefore { edit, .. }
                | LintFix::Replace { edit, .. } => {
                    edit.push(added_whitespace);
                }
                LintFix::Delete { .. } => unreachable!(),
            }
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

    let new_result = if let Some(prev_block) = prev_block
        && anchor_on != "after"
    {
        let anchor = if let Some(block) = next_block {
            // If next_block is Some, get the first segment
            block.segment().clone()
        } else {
            prev_block.segment().clone()
        };

        LintResult::new(
            anchor.into(),
            vec![LintFix::create_after(
                prev_block.segment().clone(),
                vec![added_whitespace],
                None,
            )],
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
    use sqruff_lib_core::dialects::syntax::SyntaxKind;
    use sqruff_lib_core::helpers::enter_panic;
    use sqruff_lib_core::lint_fix::LintFix;
    use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};

    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;
    use crate::core::rules::LintResult;
    use crate::core::test_functions::parse_ansi_string;
    use crate::utils::reflow::config::Spacing;
    use crate::utils::reflow::depth_map::DepthMap;
    use crate::utils::reflow::elements::ReflowBlock;
    use crate::utils::reflow::helpers::fixes_from_results;
    use crate::utils::reflow::reindent::IndentUnit;
    use crate::utils::reflow::respace::Tables;
    use crate::utils::reflow::sequence::{Filter, ReflowSequence};

    fn parse_string_with_config(sql: &str, config: &FluffConfig) -> ErasedSegment {
        let tables = Tables::default();
        let linter = Linter::new(config.clone(), None, None, false).unwrap();
        linter
            .parse_string(&tables, sql, None)
            .unwrap()
            .tree
            .unwrap()
    }

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
            let seq = ReflowSequence::from_root(&root, &config);

            let new_seq = seq.respace(&tables, strip_newlines, filter);
            assert_eq!(new_seq.raw(), raw_sql_out);
        }
    }

    #[test]
    fn test_reflow_process_spacing_duplicate_whitespace_fix_anchors() {
        // Ported from SQLFluff #8419: each duplicate-whitespace fix must anchor
        // the whitespace it removes. The fixes anchored the last segment of the
        // buffer instead of each `ws` being removed, so for three adjacent
        // whitespaces both results pointed at the final segment and the middle
        // one was never anchored.
        let tables = Tables::default();
        let w1 = SegmentBuilder::whitespace(tables.next_id(), " ");
        let w2 = SegmentBuilder::whitespace(tables.next_id(), " ");
        let w3 = SegmentBuilder::whitespace(tables.next_id(), " ");

        let (buffer, _, results) =
            super::process_spacing(&[w1.clone(), w2.clone(), w3.clone()], false);

        // w1 is kept; w2 and w3 are pruned from the returned buffer.
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0].id(), w1.id());

        // Each pruned whitespace is the anchor of its own result and delete fix,
        // and the kept one is never targeted.
        let anchors: Vec<u32> = results
            .iter()
            .flat_map(|result| {
                result
                    .anchor
                    .iter()
                    .map(|anchor| anchor.id())
                    .chain(result.fixes.iter().map(|fix| fix.anchor().id()))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(anchors.iter().filter(|&&id| id == w2.id()).count(), 2);
        assert_eq!(anchors.iter().filter(|&&id| id == w3.id()).count(), 2);
        assert!(anchors.iter().all(|&id| id != w1.id()));
    }

    #[test]
    fn test_reflow_sequence_respace_keeps_operator_in_unparsable() {
        // https://github.com/quarylabs/sqruff/issues/2624
        // When the right-hand side of a comparison fails to parse, `>=` ends up
        // inside an unparsable section. Reflow must leave that section alone
        // rather than splitting the operator into `> =`.
        let tables = Tables::default();
        let root = parse_ansi_string("SELECT 1 FROM t WHERE a >= @x");
        let config = <_>::default();
        let seq = ReflowSequence::from_root(&root, &config);

        let out = seq.respace(&tables, false, Filter::All).raw();
        assert!(out.contains(">="), "operator should stay joined: {out:?}");
        assert!(!out.contains("> ="), "operator must not be split: {out:?}");
    }

    #[test]
    fn test_reflow_respace_align_ignores_predecessor_on_another_line() {
        // Regression test for https://github.com/sqlfluff/sqlfluff/issues/8256.
        // With leading commas and the T-SQL `=` alias syntax, the first aliased
        // column is the first code on its line, so the code preceding it is the
        // trailing token of an *earlier* line (`SELECT`). Its column bore no
        // relation to where that alias starts, but it was still counted when
        // picking the alignment target, inflating the target column and padding
        // every *other* line to match. That produced two spaces after the leading
        // comma on aliased rows while non-aliased rows kept one.
        //
        // The commas must keep a single following space; alignment is achieved by
        // padding before the `=` instead.
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = tsql

[sqruff:layout:type:comma]
line_position = leading

[sqruff:layout:type:alias_operator]
spacing_before = align
align_within = select_clause
align_scope = bracketed

[sqruff:layout:type:alias_expression]
spacing_before = align
align_within = select_clause
align_scope = bracketed
"#,
            None,
        );

        let sql = "SELECT\n    FirstCol = 'a'\n    , SecondCol = 'b'\n    , ThirdCol = 'c'\n    , t.NonaliasedCol\nFROM TestTable AS t\n";

        let tables = Tables::default();
        let root = parse_string_with_config(sql, &config);
        let seq = ReflowSequence::from_root(&root, &config);
        let result = seq.respace(&tables, false, Filter::All).raw();

        // Every leading comma keeps exactly one following space.
        assert!(
            !result.contains(",  "),
            "leading comma must keep a single following space: {result:?}"
        );
        // Alignment is still achieved, via padding before the `=`.
        let operator_columns: std::collections::HashSet<usize> =
            result.lines().filter_map(|line| line.find('=')).collect();
        assert_eq!(
            operator_columns.len(),
            1,
            "all `=` operators should align to one column: {result:?}"
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum EditType {
        CreateBefore,
        CreateAfter,
        Replace,
        Delete,
    }

    fn make_reflow_block(
        segment: ErasedSegment,
        reference: &ErasedSegment,
        root: &ErasedSegment,
        config: &FluffConfig,
    ) -> ReflowBlock {
        let depth_map = DepthMap::from_parent(root);
        ReflowBlock::from_config(
            segment,
            config.reflow(),
            depth_map.get_depth_info(reference),
        )
    }

    fn fix_edit_raws(fix: &LintFix) -> Vec<String> {
        match fix {
            LintFix::CreateBefore { edit, .. }
            | LintFix::CreateAfter { edit, .. }
            | LintFix::Replace { edit, .. } => {
                edit.iter().map(|seg| seg.raw().to_string()).collect()
            }
            LintFix::Delete { .. } => Vec::new(),
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
            let seq = ReflowSequence::from_root(&root, &config);
            let pnt = seq.elements()[point_idx].as_point().unwrap();

            let (results, new_pnt) = pnt.respace_point(
                &tables,
                seq.elements()[point_idx - 1].as_block(),
                seq.elements()[point_idx + 1].as_block(),
                &root,
                Vec::new(),
                strip_newlines,
                "before",
                IndentUnit::Space(4),
                4,
            );

            assert_eq!(new_pnt.raw(), raw_point_sql_out);

            let fixes = fixes_from_results(results.into_iter())
                .map(|fix| {
                    let edit_type = match fix {
                        LintFix::CreateBefore { .. } => EditType::CreateBefore,
                        LintFix::CreateAfter { .. } => EditType::CreateAfter,
                        LintFix::Replace { .. } => EditType::Replace,
                        LintFix::Delete { .. } => EditType::Delete,
                    };
                    (edit_type, fix.anchor().raw().to_smolstr())
                })
                .collect_vec();

            assert_eq!(fixes, fixes_out);
        }
    }

    #[test]
    fn test_reflow_sequence_respace_preserves_newline_after_inline_comment() {
        let cases = ["touch:inline", "single:inline"];

        for spacing_after in cases {
            let _panic = enter_panic(format!("spacing_after={spacing_after}"));

            let config = FluffConfig::from_source(
                &format!(
                    r#"
[sqruff]
dialect = ansi

[sqruff:layout:type:inline_comment]
spacing_after = {spacing_after}
"#
                ),
                None,
            );
            let root = parse_string_with_config("select 1 -- comment\n+ 2", &config);
            let seq = ReflowSequence::from_root(&root, &config);
            let new_seq = seq.respace(&Tables::default(), false, Filter::All);

            assert_eq!(new_seq.raw(), "select 1 -- comment\n+ 2");
            assert!(new_seq.results().is_empty());
        }
    }

    #[test]
    fn test_reflow_sequence_respace_preserves_newline_before_inline_comment() {
        let config = FluffConfig::from_source(
            r#"
[sqruff]
dialect = ansi

[sqruff:layout:type:expression]
spacing_within = single:inline
"#,
            None,
        );
        let sql = "SELECT * FROM t WHERE a AND\n-- comment\nb\n";
        let root = parse_string_with_config(sql, &config);
        let seq = ReflowSequence::from_root(&root, &config);
        let new_seq = seq.respace(&Tables::default(), false, Filter::All);

        assert_eq!(new_seq.raw(), sql);
    }

    #[test]
    fn test_reflow_respace_handles_zero_length_blocks() {
        // Ported from SQLFluff #8395. A zero length block next to a `touch`
        // constraint must not panic. Templated placeholders can render to an
        // empty raw, so the check for a would-be `--` comment marker must not
        // assume that both sides of the point have at least one character to
        // inspect. (Upstream exercises this through the jinja templater, which
        // needs Python here; this builds the blocks directly instead.)
        let tables = Tables::default();
        let config = FluffConfig::default();
        let root = parse_ansi_string("select 1");
        let reference = root.get_raw_segments()[0].clone();

        let minus = SegmentBuilder::token(tables.next_id(), "-", SyntaxKind::Raw).finish();
        let empty = SegmentBuilder::token(tables.next_id(), "", SyntaxKind::Placeholder).finish();

        let minus_block = make_reflow_block(minus, &reference, &root, &config);
        let empty_block = make_reflow_block(empty, &reference, &root, &config);

        // Two sign indicators fuse into the "--" comment marker.
        assert!(super::would_start_comment(
            Some(&minus_block),
            Some(&minus_block)
        ));
        // A zero length block on either side has no characters to fuse, so it
        // can't build a marker on its own - and must not panic.
        assert!(!super::would_start_comment(
            Some(&minus_block),
            Some(&empty_block)
        ));
        assert!(!super::would_start_comment(
            Some(&empty_block),
            Some(&minus_block)
        ));
        // A point at the very start or end of the file has nothing to fuse with.
        assert!(!super::would_start_comment(None, Some(&minus_block)));
        assert!(!super::would_start_comment(Some(&minus_block), None));
    }

    #[test]
    fn test_handle_respace_inline_without_space_piggybacks_after_existing_insert() {
        let tables = Tables::default();
        let config = FluffConfig::default();
        let root = parse_ansi_string("select a+b");
        let raw_segments = root.get_raw_segments();
        let a_seg = raw_segments
            .iter()
            .find(|seg| seg.raw() == "a")
            .unwrap()
            .clone();
        let plus_seg = raw_segments
            .iter()
            .find(|seg| seg.raw() == "+")
            .unwrap()
            .clone();

        let insertion =
            SegmentBuilder::token(tables.next_id(), "__inserted__", SyntaxKind::Raw).finish();

        let prev_block = make_reflow_block(insertion.clone(), &a_seg, &root, &config);
        let next_block = make_reflow_block(plus_seg.clone(), &plus_seg, &root, &config);
        let existing_results = vec![LintResult::new(
            Some(a_seg.clone()),
            vec![LintFix::create_after(a_seg, vec![insertion.clone()], None)],
            None,
            None,
        )];

        let (segment_buffer, results, edited) = super::handle_respace_inline_without_space(
            &tables,
            Spacing::Single,
            Spacing::Single,
            Some(&prev_block),
            Some(&next_block),
            Vec::new(),
            existing_results,
            "before",
        );

        assert_eq!(
            segment_buffer
                .iter()
                .map(|seg| seg.raw().to_string())
                .collect_vec(),
            vec![" ".to_owned()]
        );
        assert!(edited);
        assert_eq!(results.len(), 1);
        assert_eq!(
            fix_edit_raws(&results[0].fixes[0]),
            vec!["__inserted__".to_owned(), " ".to_owned()]
        );
    }

    #[test]
    fn test_handle_respace_inline_without_space_piggybacks_before_existing_insert() {
        let tables = Tables::default();
        let config = FluffConfig::default();
        let root = parse_ansi_string("select a+b");
        let raw_segments = root.get_raw_segments();
        let a_seg = raw_segments
            .iter()
            .find(|seg| seg.raw() == "a")
            .unwrap()
            .clone();
        let plus_seg = raw_segments
            .iter()
            .find(|seg| seg.raw() == "+")
            .unwrap()
            .clone();

        let insertion =
            SegmentBuilder::token(tables.next_id(), "__inserted__", SyntaxKind::Raw).finish();

        let prev_block = make_reflow_block(a_seg.clone(), &a_seg, &root, &config);
        let next_block = make_reflow_block(insertion.clone(), &plus_seg, &root, &config);
        let existing_results = vec![LintResult::new(
            Some(plus_seg.clone()),
            vec![LintFix::create_before(plus_seg, vec![insertion.clone()])],
            None,
            None,
        )];

        let (segment_buffer, results, edited) = super::handle_respace_inline_without_space(
            &tables,
            Spacing::Single,
            Spacing::Single,
            Some(&prev_block),
            Some(&next_block),
            Vec::new(),
            existing_results,
            "before",
        );

        assert_eq!(
            segment_buffer
                .iter()
                .map(|seg| seg.raw().to_string())
                .collect_vec(),
            vec![" ".to_owned()]
        );
        assert!(edited);
        assert_eq!(results.len(), 1);
        assert_eq!(
            fix_edit_raws(&results[0].fixes[0]),
            vec![" ".to_owned(), "__inserted__".to_owned()]
        );
    }
}
