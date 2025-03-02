use std::cell::OnceCell;
use std::iter::zip;
use std::ops::Deref;
use std::rc::Rc;

use itertools::{Itertools, chain};
use nohash_hasher::IntMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};

use super::config::{ReflowConfig, Spacing};
use super::depth_map::DepthInfo;
use super::respace::determine_constraints;
use crate::core::rules::base::LintResult;
use crate::utils::reflow::rebreak::LinePosition;
use crate::utils::reflow::respace::{
    handle_respace_inline_with_space, handle_respace_inline_without_space, process_spacing,
};

fn get_consumed_whitespace(segment: Option<&ErasedSegment>) -> Option<String> {
    let segment = segment?;

    if segment.is_type(SyntaxKind::Placeholder) {
        None
    } else {
        // match segment.block_type.as_ref() {
        //     SyntaxKind::Literal => Some(segment.source_str),
        //     _ => None,
        // }
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReflowPointData {
    segments: Vec<ErasedSegment>,
    stats: OnceCell<IndentStats>,
    class_types: OnceCell<SyntaxSet>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReflowPoint {
    value: Rc<ReflowPointData>,
}

impl Deref for ReflowPoint {
    type Target = ReflowPointData;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref()
    }
}

impl ReflowPoint {
    pub fn new(segments: Vec<ErasedSegment>) -> Self {
        Self {
            value: Rc::new(ReflowPointData {
                segments,
                stats: OnceCell::new(),
                class_types: OnceCell::new(),
            }),
        }
    }

    pub fn raw(&self) -> String {
        self.segments.iter().map(|it| it.raw()).join("")
    }

    pub fn class_types(&self) -> &SyntaxSet {
        self.class_types.get_or_init(|| {
            self.segments
                .iter()
                .flat_map(|it| it.class_types())
                .collect()
        })
    }

    fn generate_indent_stats(segments: &[ErasedSegment]) -> IndentStats {
        let mut trough = 0;
        let mut running_sum = 0;
        let mut implicit_indents = Vec::new();

        for seg in segments {
            if seg.is_indent() {
                running_sum += seg.indent_val() as isize;

                if seg.is_type(SyntaxKind::Implicit) {
                    implicit_indents.push(running_sum);
                }
            }

            if running_sum < trough {
                trough = running_sum
            }
        }

        IndentStats {
            impulse: running_sum,
            trough,
            implicit_indents: implicit_indents.into(),
        }
    }

    pub fn get_indent_segment(&self) -> Option<ErasedSegment> {
        let mut indent = None;
        for seg in self.segments.iter().rev() {
            if seg
                .get_position_marker()
                .filter(|pos_marker| !pos_marker.is_literal())
                .is_some()
            {
                continue;
            }

            match seg.get_type() {
                SyntaxKind::Newline => return indent,
                SyntaxKind::Whitespace => {
                    indent = Some(seg.clone());
                    continue;
                }
                _ => {}
            }

            if get_consumed_whitespace(Some(seg))
                .unwrap_or_default()
                .contains('\n')
            {
                return Some(seg.clone());
            }
        }
        indent
    }

    pub(crate) fn num_newlines(&self) -> usize {
        self.segments
            .iter()
            .map(|seg| {
                let newline_in_class = seg.class_types().contains(SyntaxKind::Newline) as usize;

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
            return consumed_whitespace
                .split('\n')
                .next_back()
                .unwrap()
                .to_owned()
                .into();
        }

        if let Some(seg) = seg {
            Some(seg.raw().to_string())
        } else {
            String::new().into()
        }
    }

    pub fn indent_to(
        &self,
        tables: &Tables,
        desired_indent: &str,
        after: Option<ErasedSegment>,
        before: Option<ErasedSegment>,
        description: Option<&str>,
        source: Option<&str>,
    ) -> (Vec<LintResult>, ReflowPoint) {
        assert!(
            !desired_indent.contains('\n'),
            "Newline found in desired indent."
        );
        // Get the indent (or in the case of no newline, the last whitespace)
        let indent_seg = self.get_indent_segment();

        if indent_seg
            .as_ref()
            .filter(|indent_seg| indent_seg.is_type(SyntaxKind::Placeholder))
            .is_some()
        {
            unimplemented!()
        } else if self.num_newlines() != 0 {
            if let Some(indent_seg) = indent_seg {
                if indent_seg.raw() == desired_indent {
                    unimplemented!()
                } else if desired_indent.is_empty() {
                    let idx = self
                        .segments
                        .iter()
                        .position(|seg| seg == &indent_seg)
                        .unwrap();
                    return (
                        vec![LintResult::new(
                            indent_seg.clone().into(),
                            vec![LintFix::delete(indent_seg.clone())],
                            Some(
                                description
                                    .map_or_else(
                                        || "Line should not be indented.".to_owned(),
                                        ToOwned::to_owned,
                                    )
                                    .to_string(),
                            ),
                            source.map(|s| s.to_string()),
                        )],
                        ReflowPoint::new(
                            self.segments[..idx]
                                .iter()
                                .chain(self.segments[idx + 1..].iter())
                                .cloned()
                                .collect(),
                        ),
                    );
                };

                let new_indent =
                    indent_seg.edit(tables.next_id(), desired_indent.to_owned().into(), None);
                let idx = self
                    .segments
                    .iter()
                    .position(|it| it == &indent_seg)
                    .unwrap();

                let description = format!("Expected {}.", indent_description(desired_indent));

                let lint_result = LintResult::new(
                    indent_seg.clone().into(),
                    vec![LintFix::replace(indent_seg, vec![new_indent.clone()], None)],
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

                let new_indent = SegmentBuilder::whitespace(tables.next_id(), desired_indent);

                let (last_newline_idx, last_newline) = self
                    .segments
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, it)| {
                        it.is_type(SyntaxKind::Newline)
                            && it.get_position_marker().unwrap().is_literal()
                    })
                    .unwrap();

                let mut new_segments = self.segments[..=last_newline_idx].to_vec();
                new_segments.push(new_indent.clone());
                new_segments.extend_from_slice(&self.segments[last_newline_idx + 1..]);

                return (
                    vec![LintResult::new(
                        if let Some(before) = before {
                            before.into()
                        } else {
                            unimplemented!()
                        },
                        vec![LintFix::replace(
                            last_newline.clone(),
                            vec![last_newline.clone(), new_indent],
                            None,
                        )],
                        format!("Expected {}", indent_description(desired_indent)).into(),
                        None,
                    )],
                    ReflowPoint::new(new_segments),
                );
            }
        } else {
            // There isn't currently a newline.
            let new_newline = SegmentBuilder::newline(tables.next_id(), "\n");
            // Check for whitespace
            let ws_seg = self
                .segments
                .iter()
                .find(|seg| seg.is_type(SyntaxKind::Whitespace));

            if let Some(ws_seg) = ws_seg {
                let new_segs = if desired_indent.is_empty() {
                    vec![new_newline]
                } else {
                    vec![
                        new_newline,
                        ws_seg.edit(tables.next_id(), desired_indent.to_owned().into(), None),
                    ]
                };
                let idx = self.segments.iter().position(|it| ws_seg == it).unwrap();
                let description = if let Some(before_seg) = before {
                    format!(
                        "Expected line break and {} before {:?}.",
                        indent_description(desired_indent),
                        before_seg.raw()
                    )
                } else if let Some(after_seg) = after {
                    format!(
                        "Expected line break and {} after {:?}.",
                        indent_description(desired_indent),
                        after_seg.raw()
                    )
                } else {
                    format!(
                        "Expected line break and {}.",
                        indent_description(desired_indent)
                    )
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
                        description.into(),
                        source.map(ToOwned::to_owned),
                    )],
                    new_point,
                );
            } else {
                let new_indent = SegmentBuilder::whitespace(tables.next_id(), desired_indent);

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
                            before.clone().into(),
                            vec![fix],
                            Some(format!(
                                "Expected line break and {} before {:?}",
                                indent_description(desired_indent),
                                before.raw()
                            )),
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
                        after.raw()
                    );

                    return (
                        vec![LintResult::new(
                            Some(after),
                            vec![fix],
                            Some(description),
                            source.map(ToOwned::to_owned),
                        )],
                        ReflowPoint::new(vec![new_newline, new_indent]),
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn respace_point(
        &self,
        tables: &Tables,
        prev_block: Option<&ReflowBlock>,
        next_block: Option<&ReflowBlock>,
        root_segment: &ErasedSegment,
        lint_results: Vec<LintResult>,
        strip_newlines: bool,
        anchor_on: &'static str,
    ) -> (Vec<LintResult>, ReflowPoint) {
        let mut existing_results = lint_results;
        let (pre_constraint, post_constraint, strip_newlines) =
            determine_constraints(prev_block, next_block, strip_newlines);

        // The buffer is used to create the new reflow point to return
        let (mut segment_buffer, mut last_whitespace, mut new_results) =
            process_spacing(&self.segments, strip_newlines);

        if let Some((_, whitespace)) = next_block
            .zip(last_whitespace.clone())
            .filter(|(next_block, _)| next_block.class_types().contains(SyntaxKind::EndOfFile))
        {
            new_results.push(LintResult::new(
                None,
                vec![LintFix::delete(whitespace.clone())],
                Some("Unnecessary trailing whitespace at end of file.".into()),
                None,
            ));

            let pos = segment_buffer
                .iter()
                .position(|it| it == &whitespace)
                .unwrap();
            segment_buffer.remove(pos);

            last_whitespace = None;
        }

        if segment_buffer
            .iter()
            .any(|seg| seg.is_type(SyntaxKind::Newline))
            && !strip_newlines
            || (next_block.is_some()
                && next_block
                    .unwrap()
                    .class_types()
                    .contains(SyntaxKind::EndOfFile))
        {
            if let Some(last_whitespace) = last_whitespace {
                let ws_idx = self
                    .segments
                    .iter()
                    .position(|it| it == &last_whitespace)
                    .unwrap();
                if ws_idx > 0 {
                    let segments_slice = &self.segments[..ws_idx];

                    let prev_seg = segments_slice
                        .iter()
                        .rev()
                        .find(|seg| {
                            !matches!(seg.get_type(), SyntaxKind::Indent | SyntaxKind::Implicit)
                        })
                        .unwrap();

                    if prev_seg.is_type(SyntaxKind::Newline)
                        && prev_seg.get_end_loc() < last_whitespace.get_start_loc()
                    {
                        segment_buffer.remove(ws_idx);

                        let temp_idx = last_whitespace
                            .get_position_marker()
                            .unwrap()
                            .templated_slice
                            .start;

                        if let Some((index, _)) =
                            existing_results.iter().enumerate().find(|(_, res)| {
                                res.anchor
                                    .as_ref()
                                    .and_then(|a| a.get_position_marker())
                                    .is_some_and(|pm| pm.templated_slice.end == temp_idx)
                            })
                        {
                            let mut res = existing_results.remove(index);

                            res.fixes.push(LintFix::delete(last_whitespace));
                            let new_result = LintResult::new(res.anchor, res.fixes, None, None);
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
                tables,
                pre_constraint,
                post_constraint,
                prev_block,
                next_block,
                root_segment,
                segment_buffer,
                last_whitespace,
            );

            new_results.extend(results);
            segment_buffer
        } else {
            // No. Should we insert some?
            // NOTE: This method operates on the existing fix buffer.
            let (segment_buffer, results, _edited) = handle_respace_inline_without_space(
                tables,
                pre_constraint,
                post_constraint,
                prev_block,
                next_block,
                segment_buffer,
                chain(existing_results, new_results).collect_vec(),
                anchor_on,
            );

            existing_results = Vec::new();
            new_results = results;

            segment_buffer
        };

        existing_results.extend(new_results);
        (existing_results, ReflowPoint::new(segment_buffer))
    }

    pub fn segments(&self) -> &[ErasedSegment] {
        &self.segments
    }

    pub fn indent_impulse(&self) -> &IndentStats {
        self.stats
            .get_or_init(|| Self::generate_indent_stats(self.segments()))
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IndentStats {
    pub impulse: isize,
    pub trough: isize,
    pub implicit_indents: Rc<[isize]>,
}

impl IndentStats {
    pub fn from_combination(first: Option<IndentStats>, second: &IndentStats) -> Self {
        match first {
            Some(first_stats) => IndentStats {
                impulse: first_stats.impulse + second.impulse,
                trough: std::cmp::min(first_stats.trough, first_stats.impulse + second.trough),
                implicit_indents: second.implicit_indents.clone(),
            },
            None => second.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ReflowBlockData {
    segment: ErasedSegment,
    spacing_before: Spacing,
    spacing_after: Spacing,
    line_position: Option<Vec<LinePosition>>,
    depth_info: DepthInfo,
    stack_spacing_configs: IntMap<u64, Spacing>,
    line_position_configs: IntMap<u64, &'static str>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ReflowBlock {
    value: Rc<ReflowBlockData>,
}

impl Deref for ReflowBlock {
    type Target = ReflowBlockData;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref()
    }
}

impl ReflowBlock {
    pub fn segment(&self) -> &ErasedSegment {
        &self.segment
    }

    pub fn spacing_before(&self) -> Spacing {
        self.spacing_before
    }

    pub fn spacing_after(&self) -> Spacing {
        self.spacing_after
    }

    pub fn line_position(&self) -> Option<&[LinePosition]> {
        self.line_position.as_deref()
    }

    pub fn depth_info(&self) -> &DepthInfo {
        &self.depth_info
    }

    pub fn class_types(&self) -> &SyntaxSet {
        self.segment.class_types()
    }

    pub fn stack_spacing_configs(&self) -> &IntMap<u64, Spacing> {
        &self.stack_spacing_configs
    }

    pub fn line_position_configs(&self) -> &IntMap<u64, &'static str> {
        &self.line_position_configs
    }
}

impl ReflowBlock {
    pub fn from_config(
        segment: ErasedSegment,
        config: &ReflowConfig,
        depth_info: DepthInfo,
    ) -> Self {
        let block_config = config.get_block_config(segment.class_types(), Some(&depth_info));

        let mut stack_spacing_configs = IntMap::default();
        let mut line_position_configs = IntMap::default();

        for (hash, class_types) in zip(&depth_info.stack_hashes, &depth_info.stack_class_types) {
            let cfg = config.get_block_config(class_types, None);

            if let Some(spacing_within) = cfg.spacing_within {
                stack_spacing_configs.insert(*hash, spacing_within);
            }

            if let Some(line_position) = cfg.line_position {
                line_position_configs.insert(*hash, line_position);
            }
        }

        let line_position = block_config.line_position.map(|line_position| {
            line_position
                .split(':')
                .map(|it| it.parse().unwrap())
                .collect()
        });

        Self {
            value: Rc::new(ReflowBlockData {
                segment,
                spacing_before: block_config.spacing_before,
                spacing_after: block_config.spacing_after,
                line_position,
                depth_info,
                stack_spacing_configs,
                line_position_configs,
            }),
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

#[derive(Debug, Clone, PartialEq)]
pub enum ReflowElement {
    Block(ReflowBlock),
    Point(ReflowPoint),
}

impl ReflowElement {
    pub fn raw(&self) -> String {
        self.segments().iter().map(|it| it.raw()).join("")
    }

    pub fn segments(&self) -> &[ErasedSegment] {
        match self {
            ReflowElement::Block(block) => std::slice::from_ref(&block.segment),
            ReflowElement::Point(point) => &point.segments,
        }
    }

    pub fn class_types(&self) -> &SyntaxSet {
        match self {
            ReflowElement::Block(reflow_block) => reflow_block.class_types(),
            ReflowElement::Point(reflow_point) => reflow_point.class_types(),
        }
    }

    pub fn num_newlines(&self) -> usize {
        self.segments()
            .iter()
            .map(|seg| {
                let newline_in_class = seg.class_types().contains(SyntaxKind::Newline) as usize;

                let consumed_whitespace = get_consumed_whitespace(seg.into()).unwrap_or_default();
                newline_in_class + consumed_whitespace.matches('\n').count()
            })
            .sum()
    }

    pub fn as_point(&self) -> Option<&ReflowPoint> {
        if let Self::Point(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_block(&self) -> Option<&ReflowBlock> {
        if let Self::Block(v) = self {
            Some(v)
        } else {
            None
        }
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
