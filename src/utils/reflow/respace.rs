use itertools::{enumerate, Itertools};

use super::elements::ReflowBlock;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::{Segment, WhitespaceSegment, WhitespaceSegmentNewArgs};
use crate::core::rules::base::{EditType, LintFix, LintResult};

fn unpack_constraint(constraint: String, mut strip_newlines: bool) -> (String, bool) {
    let (constraint, modifier) = if constraint.starts_with("align") {
        (constraint.as_str(), "".into())
    } else {
        constraint
            .split_once(':')
            .map(|(left, right)| (left, Some(right)))
            .unwrap_or((constraint.as_str(), None))
    };

    match modifier {
        Some("inline") => {
            strip_newlines = true;
        }
        Some(modifier) => panic!("Unexpected constraint modifier: {modifier:?}"),
        None => {}
    }

    (constraint.into(), strip_newlines)
}

pub fn determine_constraints(
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    strip_newlines: bool,
) -> (String, String, bool) {
    // Start with the defaults
    let (pre_constraint, strip_newlines) = unpack_constraint(
        if let Some(prev_block) = prev_block {
            prev_block.spacing_after.clone()
        } else {
            "single".into()
        },
        strip_newlines,
    );

    let (post_constraint, strip_newlines) = unpack_constraint(
        if let Some(next_block) = next_block {
            next_block.spacing_before.clone()
        } else {
            "single".into()
        },
        strip_newlines,
    );

    (pre_constraint, post_constraint, strip_newlines)
}

pub fn process_spacing(
    segment_buffer: Vec<Box<dyn Segment>>,
    strip_newlines: bool,
) -> (Vec<Box<dyn Segment>>, Option<Box<dyn Segment>>, Vec<LintResult>) {
    let mut removal_buffer = Vec::new();
    let mut result_buffer = Vec::new();
    let mut last_whitespace = Vec::new();

    let mut last_iter_seg = None;

    // Loop through the existing segments looking for spacing.
    for seg in &segment_buffer {
        // If it's whitespace, store it.
        if seg.is_type("whitespace") {
            last_whitespace.push(seg.clone());
        }
        // If it's a newline, react accordingly.
        // NOTE: This should only trigger on literal newlines.
        else if matches!(seg.get_type(), "newline" | "end_of_file") {
            unimplemented!()
        }

        last_iter_seg = seg.into();
    }

    if last_whitespace.len() >= 2 {
        let seg = last_iter_seg.unwrap().clone();

        for ws in last_whitespace.iter().skip(1).cloned() {
            removal_buffer.push(ws.clone());
            result_buffer.push(LintResult::new(
                seg.clone().into(),
                vec![LintFix::delete(ws)],
                None,
                "Unnecessary trailing whitespace.".to_owned().into(),
                None,
            ));
        }
    }

    // Turn the removal buffer updated segment buffer, last whitespace and
    // associated fixes.

    let filtered_segment_buffer =
        segment_buffer.iter().filter(|s| !removal_buffer.contains(s)).cloned().collect_vec();

    let last_whitespace_option =
        if !last_whitespace.is_empty() { Some(last_whitespace[0].clone()) } else { None };

    (filtered_segment_buffer, last_whitespace_option, result_buffer)
}

fn determine_aligned_inline_spacing(
    root_segment: &dyn Segment,
    whitespace_seg: &dyn Segment,
    next_seg: &dyn Segment,
    next_pos: &PositionMarker,
    segment_type: &str,
    align_within: Option<&str>,
    align_scope: Option<&str>,
) -> String {
    unimplemented!()
}

fn extract_alignment_config(constraint: &str) -> (String, Option<String>, Option<String>) {
    unimplemented!()
}

pub fn handle_respace_inline_with_space(
    pre_constraint: String,
    post_constraint: String,
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    /* root_segment: &dyn Segment, */
    mut segment_buffer: Vec<Box<dyn Segment>>,
    last_whitespace: Box<dyn Segment>,
) -> (Vec<Box<dyn Segment>>, Vec<LintResult>) {
    // Get some indices so that we can reference around them
    let ws_idx = segment_buffer.iter().position(|it| it.dyn_eq(&*last_whitespace)).unwrap();

    if [pre_constraint.as_str(), post_constraint.as_str()].contains(&"touch") {
        segment_buffer.remove(ws_idx);

        let description = if let Some(next_block) = next_block {
            format!(
                "Unexpected whitespace before {}.",
                next_block.segments[0].get_raw().unwrap() /* Replace with appropriate function
                                                           * to get segment name */
            )
        } else {
            "Unexpected whitespace".to_string()
        };

        let lint_result = LintResult::new(
            last_whitespace.clone().into(),
            vec![LintFix::delete(last_whitespace)],
            None,
            Some(description),
            None,
        );

        // Return the segment buffer and the lint result
        return (segment_buffer, vec![lint_result]);
    }

    // Handle left alignment & singles
    if pre_constraint == "single" && post_constraint == "single" {
        let desc = if let Some(next_block) = next_block {
            format!(
                "Expected only single space before `{}`. Found {:?}.",
                &next_block.segments[0].get_raw().unwrap(),
                last_whitespace.get_raw().unwrap()
            )
        } else {
            format!("Expected only single space. Found {:?}.", last_whitespace.get_raw().unwrap())
        };

        let desired_space = " ";
        let mut new_results = Vec::new();

        if last_whitespace.get_raw().unwrap() != desired_space {
            let new_seg = last_whitespace.edit(desired_space.to_owned().into(), None);
            new_results.push(LintResult::new(
                last_whitespace.clone().into(),
                vec![LintFix {
                    edit_type: EditType::Replace,
                    anchor: last_whitespace,
                    edit: vec![new_seg.clone()].into(),
                    source: vec![],
                }],
                None,
                Some(desc),
                None,
            ));

            segment_buffer[ws_idx] = new_seg;
        }

        return (segment_buffer, new_results);
    }

    unimplemented!("Unexpected Constraints: {pre_constraint}, {post_constraint}");
}

#[allow(unused_variables)]
pub fn handle_respace_inline_without_space(
    pre_constraint: String,
    post_constraint: String,
    prev_block: Option<&ReflowBlock>,
    next_block: Option<&ReflowBlock>,
    mut segment_buffer: Vec<Box<dyn Segment>>,
    mut existing_results: Vec<LintResult>,
    anchor_on: &str,
) -> (Vec<Box<dyn Segment>>, Vec<LintResult>, bool) {
    let constraints = ["touch", "any"];

    if constraints.contains(&pre_constraint.as_str())
        || constraints.contains(&post_constraint.as_str())
    {
        return (segment_buffer, existing_results, false);
    }

    let added_whitespace =
        WhitespaceSegment::new(" ", &PositionMarker::default(), WhitespaceSegmentNewArgs {});

    // Add it to the buffer first (the easy bit). The hard bit is to then determine
    // how to generate the appropriate LintFix objects.
    segment_buffer.push(added_whitespace.clone());

    // So special handling here. If segments either side already exist then we don't
    // care which we anchor on but if one is already an insertion (as shown by a
    // lack) of pos_marker, then we should piggy back on that pre-existing fix.
    let mut existing_fix = None;
    let mut insertion = None;

    if let Some(block) = prev_block {
        if let Some(last_segment) = block.segments.last() {
            if last_segment.get_position_marker().is_none() {
                existing_fix = Some("after");
                insertion = Some(last_segment);
            }
        }
    } else if let Some(block) = next_block {
        if let Some(first_segment) = block.segments.first() {
            if first_segment.get_position_marker().is_none() {
                existing_fix = Some("before");
                insertion = Some(first_segment);
            }
        }
    }

    if let Some(existing_fix) = existing_fix {
        let mut res_found = None;
        let mut fix_found = None;

        'outer: for (result_idx, res) in enumerate(&existing_results) {
            for (fix_idx, fix) in enumerate(&res.fixes) {
                if let Some(edits) = &fix.edit {
                    if edits
                        .iter()
                        .any(|e| e.get_uuid().unwrap() == insertion.unwrap().get_uuid().unwrap())
                    {
                        res_found = Some(result_idx);
                        fix_found = Some(fix_idx);
                        break 'outer;
                    }
                }
            }
        }

        let res = res_found.unwrap();
        let fix = fix_found.unwrap();

        let fix = &mut existing_results[res].fixes[fix];

        if existing_fix == "before" {
            unimplemented!()
        } else if existing_fix == "after" {
            fix.edit.as_mut().unwrap().push(added_whitespace);
        }

        return (segment_buffer, existing_results, true);
    }

    let new_result = if let Some(prev_block) = prev_block
        && anchor_on != "after"
    {
        let anchor = if let Some(block) = next_block {
            // If next_block is Some, get the first segment
            &block.segments[0]
        } else {
            prev_block.segments.last().unwrap()
        }
        .clone();

        LintResult::new(
            anchor.into(),
            vec![LintFix {
                edit_type: EditType::CreateAfter,
                anchor: prev_block.segments.last().cloned().unwrap(),
                edit: vec![added_whitespace].into(),
                source: vec![],
            }],
            None,
            Some("insert whitespace".into()),
            None,
        )
    } else {
        unimplemented!("Not set up to handle a missing _after_ and _before_.")
    };

    existing_results.push(new_result);
    (segment_buffer, existing_results, false)
}
