use std::mem::take;

use super::config::ReflowConfig;
use super::depth_map::DepthMap;
use super::elements::{ReflowBlock, ReflowElement, ReflowPoint, ReflowSequenceType};
use crate::core::config::FluffConfig;
use crate::core::parser::segments::base::Segment;
use crate::core::rules::base::LintResult;

pub struct ReflowSequence {
    elements: ReflowSequenceType,
    lint_results: Vec<LintResult>,
}

impl ReflowSequence {
    pub fn results(self) -> Vec<LintResult> {
        self.lint_results
    }

    pub fn from_root(root_segment: Box<dyn Segment>, _config: FluffConfig) -> Self {
        Self::from_raw_segments(root_segment.get_raw_segments(), root_segment)
    }

    pub fn from_raw_segments(
        segments: Vec<Box<dyn Segment>>,
        root_segment: Box<dyn Segment>,
    ) -> Self {
        let depth_map = DepthMap::from_raws_and_root(segments.clone(), root_segment);
        let elements = Self::elements_from_raw_segments(segments, depth_map);

        Self { elements, lint_results: Vec::new() }
    }

    fn elements_from_raw_segments(
        segments: Vec<Box<dyn Segment>>,
        depth_map: DepthMap,
    ) -> Vec<ReflowElement> {
        let mut elem_buff = Vec::new();
        let mut seg_buff = Vec::new();

        for seg in segments {
            // NOTE: end_of_file is block-like rather than point-like.
            // This is to facilitate better evaluation of the ends of files.
            // NOTE: This also allows us to include literal placeholders for
            // whitespace only strings.
            if matches!(seg.get_type(), "whitespace" | "newline" | "indent") {
                // Add to the buffer and move on.
                seg_buff.push(seg);
                continue;
            } else if !elem_buff.is_empty() || !seg_buff.is_empty() {
                // There are elements. The last will have been a block.
                // Add a point before we add the block. NOTE: It may be empty.
                elem_buff.push(ReflowElement::Point(ReflowPoint { segments: seg_buff.clone() }));
            }

            let depth_info = depth_map.get_depth_info(&seg);
            // Add the block, with config info.
            elem_buff.push(ReflowElement::Block(ReflowBlock::from_config(
                vec![seg],
                ReflowConfig::default(),
                depth_info,
            )));
            // Empty the buffer
            seg_buff.clear();
        }

        elem_buff
    }

    pub fn respace(mut self) -> Self {
        let mut lint_results = take(&mut self.lint_results);
        let mut new_elements = Vec::new();

        for (point, pre, post) in self.iter_points_with_constraints() {
            let (new_lint_results, new_point) = point.respace_point(pre, post);

            lint_results.extend(new_lint_results);

            if let Some(pre_value) = pre {
                if new_elements.is_empty() || new_elements.last().unwrap() != pre_value {
                    new_elements.push(pre_value.clone().into());
                }
            }

            new_elements.push(new_point.into());

            if let Some(post) = post {
                new_elements.push(post.clone().into());
            }
        }

        self.elements = new_elements;
        self.lint_results = lint_results;
        self
    }

    fn iter_points_with_constraints(
        &self,
    ) -> impl Iterator<Item = (&ReflowPoint, Option<&ReflowBlock>, Option<&ReflowBlock>)> + '_ {
        self.elements.iter().enumerate().flat_map(|(idx, elem)| {
            if let ReflowElement::Point(elem) = elem {
                {
                    let mut pre = None;
                    let mut post = None;

                    if idx > 0 {
                        if let ReflowElement::Block(ref block) = self.elements[idx - 1] {
                            pre = Some(block);
                        }
                    }

                    if idx < self.elements.len() - 1 {
                        if let ReflowElement::Block(ref block) = self.elements[idx + 1] {
                            post = Some(block);
                        }
                    }

                    (elem, pre, post).into()
                }
            } else {
                None
            }
        })
    }
}
