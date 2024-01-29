use std::collections::{HashMap, HashSet};
use std::iter::zip;

use itertools::{chain, Itertools};

use super::config::ReflowConfig;
use super::depth_map::DepthInfo;
use super::respace::determine_constraints;
use crate::core::parser::segments::base::Segment;
use crate::core::rules::base::LintResult;
use crate::utils::reflow::respace::{
    handle_respace_inline_with_space, handle_respace_inline_without_space, process_spacing,
};

#[derive(Debug, Clone, Default)]
pub struct ReflowPoint {
    pub segments: Vec<Box<dyn Segment>>,
}

impl ReflowPoint {
    pub fn respace_point(
        &self,
        prev_block: Option<&ReflowBlock>,
        next_block: Option<&ReflowBlock>,
        lint_results: Vec<LintResult>,
    ) -> (Vec<LintResult>, ReflowPoint) {
        let mut existing_results = lint_results;
        let (pre_constraint, post_constraint, strip_newlines) =
            determine_constraints(prev_block, next_block, false);

        // The buffer is used to create the new reflow point to return
        let (segment_buffer, last_whitespace, mut new_results) =
            process_spacing(self.segments.clone(), strip_newlines);

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
        (existing_results, ReflowPoint { segments: segment_buffer })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ReflowBlock {
    pub spacing_before: String,
    pub spacing_after: String,
    pub segments: Vec<Box<dyn Segment>>,
    pub depth_info: DepthInfo,
    pub stack_spacing_configs: HashMap<u64, String>,
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
            spacing_before: block_config.spacing_before,
            spacing_after: block_config.spacing_after,
            stack_spacing_configs,
            segments,
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
