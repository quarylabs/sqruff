use itertools::{enumerate, Itertools};

use super::elements::ReflowBlock;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::{Segment, WhitespaceSegment, WhitespaceSegmentNewArgs};
use crate::core::rules::base::{EditType, LintFix, LintResult};

fn unpack_constraint(constraint: &str, mut strip_newlines: bool) -> (String, bool) {
    let (constraint, modifier) = if constraint.starts_with("align") {
        (constraint, "".into())
    } else {
        constraint
            .split_once(':')
            .map(|(left, right)| (left, Some(right)))
            .unwrap_or((constraint, None))
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
    let (mut pre_constraint, strip_newlines) = unpack_constraint(
        if let Some(prev_block) = prev_block { &prev_block.spacing_after } else { "single".into() },
        strip_newlines,
    );

    let (mut post_constraint, mut strip_newlines) = unpack_constraint(
        if let Some(next_block) = next_block {
            &next_block.spacing_before
        } else {
            "single".into()
        },
        strip_newlines,
    );

    let mut within_spacing = String::new();
    let mut idx = None;

    if let Some((prev_block, next_block)) = prev_block.zip(next_block) {
        let common = prev_block.depth_info.common_with(&next_block.depth_info);
        let last_common = common.last().unwrap();
        idx = prev_block
            .depth_info
            .stack_hashes
            .iter()
            .position(|p| p == last_common)
            .unwrap()
            .into();

        let within_constraint = prev_block.stack_spacing_configs.get(last_common);
        if let Some(within_constraint) = within_constraint {
            (within_spacing, strip_newlines) = unpack_constraint(within_constraint, strip_newlines);
        }
    }

    match within_spacing.as_str() {
        "touch" => {
            if pre_constraint != "any" {
                pre_constraint = "touch".to_string();
            }
            if post_constraint != "any" {
                post_constraint = "touch".to_string();
            }
        }
        "any" => {
            pre_constraint = "any".to_string();
            post_constraint = "any".to_string();
        }
        "single" => {}
        _ if !within_spacing.is_empty() => {
            panic!(
                "Unexpected within constraint: '{}' for {:?}",
                within_spacing,
                prev_block.unwrap().depth_info.stack_class_types[idx.unwrap()]
            );
        }
        _ => {}
    }

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
            if seg.get_position_marker().is_some_and(|pos_marker| !pos_marker.is_literal()) {
                last_whitespace = Vec::new();
                continue;
            }

            if strip_newlines && seg.is_type("newline") {
                removal_buffer.push(seg.clone());
                result_buffer.push(LintResult::new(
                    seg.clone().into(),
                    vec![LintFix::delete(seg.clone())],
                    None,
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
                        None,
                        Some("Unnecessary trailing whitespace.".into()),
                        None,
                    ))
                }
            }
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

    if ["any"].contains(&pre_constraint.as_str()) || ["any"].contains(&post_constraint.as_str()) {
        return (segment_buffer, vec![]);
    }

    if [pre_constraint.as_str(), post_constraint.as_str()].contains(&"touch") {
        segment_buffer.remove(ws_idx);

        let description = if let Some(next_block) = next_block {
            format!("Unexpected whitespace before {:?}.", next_block.segments[0].get_raw().unwrap())
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
                "Expected only single space before {:?}. Found {:?}.",
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

    let desc = if let Some((prev_block, next_block)) = prev_block.zip(next_block) {
        format!(
            "Expected single whitespace between {:?} and {:?}.",
            prev_block.segments.last().unwrap().get_raw().unwrap(),
            next_block.segments[0].get_raw().unwrap()
        )
    } else {
        format!("Expected single whitespace.")
    };

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
            desc.into(),
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

    use crate::core::parser::segments::test_functions::parse_ansi_string;
    use crate::core::rules::base::EditType;
    use crate::helpers::enter_panic;
    use crate::utils::reflow::helpers::fixes_from_results;
    use crate::utils::reflow::sequence::{Filter, ReflowSequence};

    #[test]
    fn test_reflow__sequence_respace() {
        let cases = [
            // Basic cases
            ("select 1+2", (false, Filter::All), "select 1 + 2"),
            ("select    1   +   2    ", (false, Filter::All), "select 1 + 2"),
            // Check newline handling
            ("select\n    1   +   2", (false, Filter::All), "select\n    1 + 2"),
            ("select\n    1   +   2", (true, Filter::All), "select 1 + 2"),
            // Check filtering
            ("select  \n  1   +   2 \n ", (false, Filter::All), "select\n  1 + 2\n"),
            ("select  \n  1   +   2 \n ", (false, Filter::Inline), "select  \n  1 + 2 \n "),
            ("select  \n  1   +   2 \n ", (false, Filter::Newline), "select\n  1   +   2\n"),
        ];

        for (raw_sql_in, (strip_newlines, filter), raw_sql_out) in cases {
            let root = parse_ansi_string(raw_sql_in);
            let seq = ReflowSequence::from_root(root, <_>::default());

            let new_seq = seq.respace(strip_newlines, filter);
            assert_eq!(new_seq.raw(), raw_sql_out);
        }
    }

    #[test]
    fn test_reflow__point_respace_point() {
        let cases = [
            // Basic cases
            ("select    1", 1, false, " ", vec![(EditType::Replace, "    ".to_owned())]),
            ("select 1+2", 3, false, " ", vec![(EditType::CreateAfter, "1".to_owned())]),
            ("select (1+2)", 3, false, "", vec![]),
            ("select (  1+2)", 3, false, "", vec![(EditType::Delete, "  ".to_owned())]),
            // Newline handling
            ("select\n1", 1, false, "\n", vec![]),
            ("select\n  1", 1, false, "\n  ", vec![]),
            ("select  \n  1", 1, false, "\n  ", vec![(EditType::Delete, "  ".to_owned())]),
            (
                "select  \n 1",
                1,
                true,
                " ",
                vec![
                    (EditType::Delete, "\n".to_owned()),
                    (EditType::Delete, " ".to_owned()),
                    (EditType::Replace, "  ".to_owned()),
                ],
            ),
            (
                "select ( \n  1)",
                3,
                true,
                "",
                vec![
                    (EditType::Delete, "\n".to_owned()),
                    (EditType::Delete, "  ".to_owned()),
                    (EditType::Delete, " ".to_owned()),
                ],
            ),
        ];

        for (raw_sql_in, point_idx, strip_newlines, raw_point_sql_out, fixes_out) in cases {
            let _panic = enter_panic(format!("{raw_sql_in:?}"));

            let root = parse_ansi_string(raw_sql_in);
            let seq = ReflowSequence::from_root(root, <_>::default());
            let pnt = seq.elements()[point_idx].as_point().unwrap();

            let (results, new_pnt) = pnt.respace_point(
                seq.elements()[point_idx - 1].as_block(),
                seq.elements()[point_idx + 1].as_block(),
                Vec::new(),
                strip_newlines,
            );

            assert_eq!(new_pnt.raw(), raw_point_sql_out);

            let fixes = fixes_from_results(results.into_iter())
                .into_iter()
                .map(|fix| (fix.edit_type, fix.anchor.get_raw().unwrap()))
                .collect_vec();

            assert_eq!(fixes, fixes_out);
        }
    }
}
