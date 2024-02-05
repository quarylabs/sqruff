use std::collections::{HashMap, HashSet};
use std::iter::zip;

use itertools::{chain, Itertools};

use super::config::ReflowConfig;
use super::depth_map::DepthInfo;
use super::respace::determine_constraints;
use crate::core::parser::segments::base::{NewlineSegment, Segment};
use crate::core::parser::segments::meta::Indent;
use crate::core::rules::base::{LintFix, LintResult};
use crate::utils::reflow::respace::{
    handle_respace_inline_with_space, handle_respace_inline_without_space, process_spacing,
};

fn get_consumed_whitespace(segment: Option<&dyn Segment>) -> Option<String> {
    let Some(segment) = segment else {
        return None;
    };

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
    pub segments: Vec<Box<dyn Segment>>,
    pub stats: IndentStats,
}

impl ReflowPoint {
    pub fn new(segments: Vec<Box<dyn Segment>>) -> Self {
        let stats = Self::generate_indent_stats(&segments);
        Self { segments, stats }
    }

    pub fn class_types(&self) -> HashSet<String> {
        ReflowElement::class_types(&self.segments)
    }

    fn generate_indent_stats(segments: &[Box<dyn Segment>]) -> IndentStats {
        let mut trough = 0;
        let mut running_sum = 0;
        let mut implicit_indents = Vec::new();

        for seg in segments {
            if let Some(indent) = seg.as_any().downcast_ref::<Indent>() {
                running_sum += indent.indent_val;

                if indent.is_implicit {
                    implicit_indents.push(running_sum);
                }
            }

            if running_sum < trough {
                trough = running_sum
            }
        }

        IndentStats { impulse: running_sum, trough, implicit_indents }
    }

    pub fn get_indent_segment(&self) -> Option<Box<dyn Segment>> {
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
                    if get_consumed_whitespace(seg.as_ref().into())
                        .unwrap_or_default()
                        .contains('\n')
                    {
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

                let consumed_whitespace =
                    get_consumed_whitespace(seg.as_ref().into()).unwrap_or_default();
                newline_in_class + consumed_whitespace.matches('\n').count()
            })
            .sum()
    }

    pub fn get_indent(&self) -> Option<String> {
        if self.num_newlines() == 0 {
            return None;
        }

        let seg = self.get_indent_segment();
        let consumed_whitespace = get_consumed_whitespace(seg.as_deref());

        if let Some(consumed_whitespace) = consumed_whitespace {
            return consumed_whitespace.split("\n").last().unwrap().to_owned().into();
        }

        if let Some(seg) = seg { seg.get_raw() } else { String::new().into() }
    }

    pub fn indent_to(
        &self,
        desired_indent: &str,
        after: Option<Box<dyn Segment>>,
        before: Option<Box<dyn Segment>>,
        description: Option<&str>,
        source: Option<&str>,
    ) -> (Vec<LintResult>, ReflowPoint) {
        assert!(!desired_indent.contains('\n'), "Newline found in desired indent.");
        // Get the indent (or in the case of no newline, the last whitespace)
        let indent_seg = self.get_indent_segment();

        if let Some(indent_seg) = indent_seg
            && indent_seg.is_type("placeholder")
        {
            unimplemented!()
        } else if self.num_newlines() != 0 {
            unimplemented!()
        } else {
            // There isn't currently a newline.
            let new_newline = NewlineSegment::new("\n", &<_>::default(), <_>::default());
            // Check for whitespace
            let ws_seg = self.segments.iter().find(|seg| seg.is_type("whitespace"));

            if let Some(ws_seg) = ws_seg {
                let new_segs = if desired_indent == "" {
                    vec![new_newline]
                } else {
                    vec![new_newline, ws_seg.edit(desired_indent.to_owned().into(), None)]
                };
                let idx = self.segments.iter().position(|it| ws_seg.dyn_eq(it.as_ref())).unwrap();
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
                unimplemented!()
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
        let (segment_buffer, last_whitespace, mut new_results) =
            process_spacing(self.segments.clone(), strip_newlines);

        if segment_buffer.iter().any(|seg| seg.is_type("newline")) && !strip_newlines
            || (next_block.is_some()
                && next_block.unwrap().class_types().contains(&"end_of_file".to_string()))
        {
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
            let (segment_buffer, results, edited) = handle_respace_inline_without_space(
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
        _ if indent.contains(" ") && indent.contains("\t") => "mixed indent".to_string(),
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
    pub impulse: usize,
    pub trough: usize,
    pub implicit_indents: Vec<usize>,
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
    pub segments: Vec<Box<dyn Segment>>,
    pub spacing_before: String,
    pub spacing_after: String,
    pub line_position: Option<String>,
    pub depth_info: DepthInfo,
    pub stack_spacing_configs: HashMap<u64, String>,
}

impl ReflowBlock {
    fn class_types(&self) -> HashSet<String> {
        ReflowElement::class_types(&self.segments)
    }
}

impl ReflowBlock {
    pub fn from_config(
        segments: Vec<Box<dyn Segment>>,
        config: ReflowConfig,
        depth_info: DepthInfo,
    ) -> Self {
        let block_config =
            config.get_block_config(&ReflowElement::class_types(&segments), Some(&depth_info));

        let mut stack_spacing_configs = HashMap::new();
        let mut line_position_configs = HashMap::new();

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
pub enum ReflowElement {
    Block(ReflowBlock),
    Point(ReflowPoint),
}

impl ReflowElement {
    pub fn segments(&self) -> &[Box<dyn Segment>] {
        match self {
            ReflowElement::Block(block) => &block.segments,
            ReflowElement::Point(point) => &point.segments,
        }
    }

    pub fn class_types1(&self) -> HashSet<String> {
        Self::class_types(self.segments())
    }

    pub fn num_newlines(&self) -> usize {
        self.segments()
            .iter()
            .map(|seg| {
                let newline_in_class = seg.class_types().iter().any(|ct| ct == "newline") as usize;

                let consumed_whitespace =
                    get_consumed_whitespace(seg.as_ref().into()).unwrap_or_default();
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
    pub fn class_types(segments: &[Box<dyn Segment>]) -> HashSet<String> {
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
