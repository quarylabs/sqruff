use std::iter::zip;

use ahash::{AHashMap, AHashSet};
use itertools::{chain, Itertools};

use super::config::ReflowConfig;
use super::depth_map::DepthInfo;
use super::respace::determine_constraints;
use crate::core::parser::segments::base::{
    ErasedSegment, NewlineSegment, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::meta::{Indent, MetaSegmentKind};
use crate::core::rules::base::{LintFix, LintResult};
use crate::utils::reflow::respace::{
    handle_respace_inline_with_space, handle_respace_inline_without_space, process_spacing,
};

fn get_consumed_whitespace(segment: Option<&ErasedSegment>) -> Option<String> {
    let segment = segment?;

    if segment.is_type("placeholder") {
        None
    } else {
        // match segment.block_type.as_ref() {
        //     "literal" => Some(segment.source_str),
        //     _ => None,
        // }
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReflowPoint {
    pub segments: Vec<ErasedSegment>,
    pub stats: IndentStats,
}

impl ReflowPoint {
    pub fn new(segments: Vec<ErasedSegment>) -> Self {
        let stats = Self::generate_indent_stats(&segments);
        Self { segments, stats }
    }

    pub fn raw(&self) -> String {
        self.segments.iter().map(|it| it.get_raw().unwrap()).join("")
    }

    pub fn class_types(&self) -> AHashSet<String> {
        ReflowElement::class_types(&self.segments)
    }

    fn generate_indent_stats(segments: &[ErasedSegment]) -> IndentStats {
        let mut trough = 0;
        let mut running_sum = 0;
        let mut implicit_indents = Vec::new();

        for seg in segments {
            if let Some(indent) = seg.as_any().downcast_ref::<Indent>() {
                running_sum += indent.indent_val() as isize;

                // FIXME:
                if indent.is_implicit() {
                    implicit_indents.push(running_sum);
                }
            }

            if running_sum < trough {
                trough = running_sum
            }
        }

        IndentStats { impulse: running_sum, trough, implicit_indents }
    }

    pub fn get_indent_segment(&self) -> Option<ErasedSegment> {
        let mut indent = None;

        for seg in self.segments.iter().rev() {
            match &seg.get_position_marker() {
                Some(marker) if !marker.is_literal() => continue,
                _ => (),
            }

            match seg.get_type() {
                "newline" => return indent,
                "whitespace" => indent = Some(seg.clone()),
                _ => {
                    if get_consumed_whitespace(Some(seg)).unwrap_or_default().contains('\n') {
                        return Some(seg.clone());
                    }
                }
            }
        }

        None
    }

    fn num_newlines(&self) -> usize {
        self.segments
            .iter()
            .map(|seg| {
                let newline_in_class = seg.class_types().iter().any(|ct| ct == "newline") as usize;

                let consumed_whitespace = get_consumed_whitespace(seg.into()).unwrap_or_default();
                newline_in_class + consumed_whitespace.matches('\n').count()
            })
            .sum()
    }

    pub fn get_indent(&self) -> Option<String> {
        if self.num_newlines() == 0 {
            return None;
        }

        let seg = self.get_indent_segment();
        let consumed_whitespace = get_consumed_whitespace(seg.as_ref());

        if let Some(consumed_whitespace) = consumed_whitespace {
            return consumed_whitespace.split('\n').last().unwrap().to_owned().into();
        }

        if let Some(seg) = seg { seg.get_raw() } else { String::new().into() }
    }

    pub fn indent_to(
        &self,
        desired_indent: &str,
        after: Option<ErasedSegment>,
        before: Option<ErasedSegment>,
        _description: Option<&str>,
        source: Option<&str>,
    ) -> (Vec<LintResult>, ReflowPoint) {
        assert!(!desired_indent.contains('\n'), "Newline found in desired indent.");
        // Get the indent (or in the case of no newline, the last whitespace)
        let indent_seg = self.get_indent_segment();

        if let Some(indent_seg) = &indent_seg
            && indent_seg.is_type("placeholder")
        {
            unimplemented!()
        } else if self.num_newlines() != 0 {
            if let Some(indent_seg) = indent_seg {
                if indent_seg.get_raw().unwrap() == desired_indent {
                    unimplemented!()
                } else if desired_indent.is_empty() {
                    // unimplemented!()
                };

                let new_indent = indent_seg.edit(desired_indent.to_owned().into(), None);
                let idx = self.segments.iter().position(|it| it == &indent_seg).unwrap();

                let description = format!("Expected {}.", indent_description(desired_indent));
                let lint_result = LintResult::new(
                    indent_seg.clone().into(),
                    vec![LintFix::replace(indent_seg, vec![new_indent.clone()], None)],
                    None,
                    description.into(),
                    None,
                );

                let mut new_segments = Vec::new();
                new_segments.extend_from_slice(&self.segments[..idx]);
                new_segments.push(new_indent);
                new_segments.extend_from_slice(&self.segments[idx + 1..]);

                let new_reflow_point = ReflowPoint::new(new_segments);

                (vec![lint_result], new_reflow_point)
            } else {
                if desired_indent.is_empty() {
                    return (Vec::new(), self.clone());
                }

                let _new_indent = WhitespaceSegment::create(
                    desired_indent,
                    &<_>::default(),
                    WhitespaceSegmentNewArgs,
                );

                return (
                    vec![LintResult::new(
                        if let Some(before) = before { before.into() } else { unimplemented!() },
                        vec![],
                        None,
                        format!("Expected {}", indent_description(desired_indent)).into(),
                        None,
                    )],
                    ReflowPoint::new(vec![]),
                );
            }
        } else {
            // There isn't currently a newline.
            let new_newline = NewlineSegment::create("\n", &<_>::default(), <_>::default());
            // Check for whitespace
            let ws_seg = self.segments.iter().find(|seg| seg.is_type("whitespace"));

            if let Some(ws_seg) = ws_seg {
                let new_segs = if desired_indent.is_empty() {
                    vec![new_newline]
                } else {
                    vec![new_newline, ws_seg.edit(desired_indent.to_owned().into(), None)]
                };
                let idx = self.segments.iter().position(|it| ws_seg == it).unwrap();
                let description = if let Some(before_seg) = before {
                    format!(
                        "Expected line break and {} before {:?}.",
                        indent_description(desired_indent),
                        before_seg.get_raw().unwrap()
                    )
                } else if let Some(after_seg) = after {
                    format!(
                        "Expected line break and {} after {:?}.",
                        indent_description(desired_indent),
                        after_seg.get_raw().unwrap()
                    )
                } else {
                    format!("Expected line break and {}.", indent_description(desired_indent))
                };

                let fix = LintFix::replace(ws_seg.clone(), new_segs.clone(), None);
                let new_point = ReflowPoint::new({
                    let mut new_segments = Vec::new();

                    // Add elements before the specified index
                    if idx > 0 {
                        new_segments.extend_from_slice(&self.segments[..idx]);
                    }

                    // Add new segments
                    new_segments.extend(new_segs);

                    // Add remaining elements after the specified index
                    if idx < self.segments.len() {
                        new_segments.extend_from_slice(&self.segments[idx + 1..]);
                    }

                    new_segments
                });

                return (
                    vec![LintResult::new(
                        ws_seg.clone().into(),
                        vec![fix],
                        None,
                        description.into(),
                        source.map(ToOwned::to_owned),
                    )],
                    new_point,
                );
            } else {
                let new_indent = WhitespaceSegment::create(
                    desired_indent,
                    &<_>::default(),
                    WhitespaceSegmentNewArgs,
                );

                if before.is_none() && after.is_none() {
                    unimplemented!(
                        "Not set up to handle empty points in this scenario without provided \
                         before/after anchor: {:?}",
                        self.segments
                    );
                } else if let Some(before) = before {
                    let fix = LintFix::create_before(
                        before.clone(),
                        vec![new_newline.clone(), new_indent.clone()],
                    );

                    return (
                        vec![LintResult::new(
                            before.into(),
                            vec![fix],
                            None,
                            None,
                            source.map(ToOwned::to_owned),
                        )],
                        ReflowPoint::new(vec![new_newline, new_indent]),
                    );
                } else {
                    let after = after.unwrap();
                    let fix = LintFix::create_after(
                        after.clone(),
                        vec![new_newline.clone(), new_indent.clone()],
                        None,
                    );
                    let description = format!(
                        "Expected line break and {} after {:?}.",
                        indent_description(desired_indent),
                        after.get_raw().unwrap()
                    );

                    return (
                        vec![LintResult::new(
                            before,
                            vec![fix],
                            None,
                            Some(description),
                            source.map(ToOwned::to_owned),
                        )],
                        ReflowPoint::new(vec![new_newline, new_indent]),
                    );
                }
            }
        }
    }

    pub fn respace_point(
        &self,
        prev_block: Option<&ReflowBlock>,
        next_block: Option<&ReflowBlock>,
        lint_results: Vec<LintResult>,
        strip_newlines: bool,
    ) -> (Vec<LintResult>, ReflowPoint) {
        let mut existing_results = lint_results;
        let (pre_constraint, post_constraint, strip_newlines) =
            determine_constraints(prev_block, next_block, strip_newlines);

        // The buffer is used to create the new reflow point to return
        let (mut segment_buffer, mut last_whitespace, mut new_results) =
            process_spacing(self.segments.clone(), strip_newlines);

        if let Some((next_block, whitespace)) = next_block.zip(last_whitespace.clone())
            && next_block.class_types().contains("end_of_file")
        {
            new_results.push(LintResult::new(
                None,
                vec![LintFix::delete(whitespace.clone())],
                None,
                Some("Unnecessary trailing whitespace at end of file.".into()),
                None,
            ));

            let pos = segment_buffer.iter().position(|it| it == &whitespace).unwrap();
            segment_buffer.remove(pos);

            last_whitespace = None;
        }

        if segment_buffer.iter().any(|seg| seg.is_type("newline")) && !strip_newlines
            || (next_block.is_some()
                && next_block.unwrap().class_types().contains(&"end_of_file".to_string()))
        {
            if let Some(last_whitespace) = last_whitespace {
                let ws_idx = self.segments.iter().position(|it| it == &last_whitespace).unwrap();
                if ws_idx > 0 {
                    let segments_slice = &self.segments[..ws_idx];

                    let prev_seg =
                        segments_slice.iter().rev().find(|seg| !seg.is_type("indent")).unwrap();

                    if prev_seg.is_type("newline")
                        && prev_seg.get_end_loc() < last_whitespace.get_start_loc()
                    {
                        segment_buffer.remove(ws_idx);

                        let temp_idx =
                            last_whitespace.get_position_marker().unwrap().templated_slice.start;

                        if let Some((index, _)) =
                            existing_results.iter().enumerate().find(|(_, res)| {
                                res.anchor
                                    .as_ref()
                                    .and_then(|a| a.get_position_marker())
                                    .map_or(false, |pm| pm.templated_slice.end == temp_idx)
                            })
                        {
                            let mut res = existing_results.remove(index);

                            res.fixes.push(LintFix::delete(last_whitespace));
                            let new_result =
                                LintResult::new(res.anchor, res.fixes, None, None, None);
                            new_results.push(new_result);
                        } else {
                            panic!("Could not find removal result.");
                        }
                    }
                }
            }

            existing_results.extend(new_results);
            return (existing_results, ReflowPoint::new(segment_buffer));
        }

        // Do we at least have _some_ whitespace?
        let segment_buffer = if let Some(last_whitespace) = last_whitespace {
            // We do - is it the right size?
            let (segment_buffer, results) = handle_respace_inline_with_space(
                pre_constraint,
                post_constraint,
                prev_block,
                next_block,
                segment_buffer,
                last_whitespace,
            );

            new_results.extend(results);
            segment_buffer
        } else {
            // No. Should we insert some?
            // NOTE: This method operates on the existing fix buffer.
            let (segment_buffer, results, _edited) = handle_respace_inline_without_space(
                pre_constraint,
                post_constraint,
                prev_block,
                next_block,
                segment_buffer,
                chain(existing_results, new_results).collect_vec(),
                "before",
            );

            existing_results = Vec::new();
            new_results = results;

            segment_buffer
        };

        existing_results.extend(new_results);
        (existing_results, ReflowPoint::new(segment_buffer))
    }

    pub fn indent_impulse(&self) -> IndentStats {
        self.stats.clone()
    }
}

fn indent_description(indent: &str) -> String {
    match indent {
        "" => "no indent".to_string(),
        _ if indent.contains(' ') && indent.contains('\t') => "mixed indent".to_string(),
        _ if indent.starts_with(' ') => {
            assert!(indent.chars().all(|c| c == ' '));
            format!("indent of {} spaces", indent.len())
        }
        _ if indent.starts_with('\t') => {
            assert!(indent.chars().all(|c| c == '\t'));
            format!("indent of {} tabs", indent.len())
        }
        _ => panic!("Invalid indent construction: {:?}", indent),
    }
}

#[derive(Debug, Clone, Default)]
pub struct IndentStats {
    pub impulse: isize,
    pub trough: isize,
    pub implicit_indents: Vec<isize>,
}

impl IndentStats {
    pub fn from_combination(first: Option<IndentStats>, second: IndentStats) -> Self {
        match first {
            Some(first_stats) => IndentStats {
                impulse: first_stats.impulse + second.impulse,
                trough: std::cmp::min(first_stats.trough, first_stats.impulse + second.trough),
                implicit_indents: second.implicit_indents,
            },
            None => second.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ReflowBlock {
    pub segments: Vec<ErasedSegment>,
    pub spacing_before: String,
    pub spacing_after: String,
    pub line_position: Option<String>,
    pub depth_info: DepthInfo,
    pub stack_spacing_configs: AHashMap<u64, String>,
}

impl ReflowBlock {
    pub fn class_types(&self) -> AHashSet<String> {
        ReflowElement::class_types(&self.segments)
    }
}

impl ReflowBlock {
    pub fn from_config(
        segments: Vec<ErasedSegment>,
        config: &ReflowConfig,
        depth_info: DepthInfo,
    ) -> Self {
        let block_config =
            config.get_block_config(&ReflowElement::class_types(&segments), Some(&depth_info));

        let mut stack_spacing_configs = AHashMap::new();
        let mut line_position_configs = AHashMap::new();

        for (hash, class_types) in zip(&depth_info.stack_hashes, &depth_info.stack_class_types) {
            let cfg = config.get_block_config(class_types, None);

            if let Some(spacing_within) = cfg.spacing_within {
                stack_spacing_configs.insert(*hash, spacing_within);
            }

            if let Some(line_position) = cfg.line_position {
                line_position_configs.insert(hash, line_position);
            }
        }

        Self {
            segments,
            spacing_before: block_config.spacing_before,
            spacing_after: block_config.spacing_after,
            line_position: block_config.line_position,
            stack_spacing_configs,
            depth_info,
        }
    }
}

impl From<ReflowBlock> for ReflowElement {
    fn from(value: ReflowBlock) -> Self {
        Self::Block(value)
    }
}

impl From<ReflowPoint> for ReflowElement {
    fn from(value: ReflowPoint) -> Self {
        Self::Point(value)
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ReflowElement {
    Block(ReflowBlock),
    Point(ReflowPoint),
}

impl ReflowElement {
    pub fn raw(&self) -> String {
        self.segments().iter().map(|it| it.get_raw().unwrap()).join("")
    }

    pub fn segments(&self) -> &[ErasedSegment] {
        match self {
            ReflowElement::Block(block) => &block.segments,
            ReflowElement::Point(point) => &point.segments,
        }
    }

    pub fn class_types1(&self) -> AHashSet<String> {
        Self::class_types(self.segments())
    }

    pub fn num_newlines(&self) -> usize {
        self.segments()
            .iter()
            .map(|seg| {
                let newline_in_class = seg.class_types().iter().any(|ct| ct == "newline") as usize;

                let consumed_whitespace = get_consumed_whitespace(seg.into()).unwrap_or_default();
                newline_in_class + consumed_whitespace.matches('\n').count()
            })
            .sum()
    }

    pub fn as_point(&self) -> Option<&ReflowPoint> {
        if let Self::Point(v) = self { Some(v) } else { None }
    }

    pub fn as_block(&self) -> Option<&ReflowBlock> {
        if let Self::Block(v) = self { Some(v) } else { None }
    }
}

impl ReflowElement {
    pub fn class_types(segments: &[ErasedSegment]) -> AHashSet<String> {
        segments.iter().flat_map(|seg| seg.combined_types()).collect()
    }
}

impl PartialEq<ReflowBlock> for ReflowElement {
    fn eq(&self, other: &ReflowBlock) -> bool {
        match self {
            ReflowElement::Block(this) => this == other,
            ReflowElement::Point(_) => false,
        }
    }
}

pub type ReflowSequenceType = Vec<ReflowElement>;
