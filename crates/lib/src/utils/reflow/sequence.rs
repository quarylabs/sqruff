use std::mem::take;

use itertools::Itertools;

use super::config::ReflowConfig;
use super::depth_map::DepthMap;
use super::elements::{ReflowBlock, ReflowElement, ReflowPoint, ReflowSequenceType};
use super::rebreak::rebreak_sequence;
use super::reindent::{construct_single_indent, lint_indent_points};
use crate::core::config::FluffConfig;
use crate::core::parser::segments::base::Segment;
use crate::core::rules::base::{LintFix, LintResult};

pub struct ReflowSequence {
    root_segment: Box<dyn Segment>,
    elements: ReflowSequenceType,
    lint_results: Vec<LintResult>,
}

impl ReflowSequence {
    pub fn raw(&self) -> String {
        self.elements.iter().map(|it| it.raw()).join("")
    }

    pub fn results(self) -> Vec<LintResult> {
        self.lint_results
    }

    pub fn fixes(self) -> Vec<LintFix> {
        self.results().into_iter().flat_map(|result| result.fixes).collect()
    }

    pub fn from_root(root_segment: Box<dyn Segment>, config: FluffConfig) -> Self {
        let depth_map = DepthMap::from_parent(&*root_segment).into();

        Self::from_raw_segments(root_segment.get_raw_segments(), root_segment, config, depth_map)
    }

    pub fn from_raw_segments(
        segments: Vec<Box<dyn Segment>>,
        root_segment: Box<dyn Segment>,
        config: FluffConfig,
        depth_map: Option<DepthMap>,
    ) -> Self {
        let reflow_config = ReflowConfig::from_fluff_config(config);
        let depth_map = depth_map.unwrap_or_else(|| {
            DepthMap::from_raws_and_root(segments.clone(), root_segment.clone())
        });
        let elements = Self::elements_from_raw_segments(segments, depth_map, reflow_config);

        Self { root_segment, elements, lint_results: Vec::new() }
    }

    fn elements_from_raw_segments(
        segments: Vec<Box<dyn Segment>>,
        depth_map: DepthMap,
        reflow_config: ReflowConfig,
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
                elem_buff.push(ReflowElement::Point(ReflowPoint::new(seg_buff.clone())));
            }

            let depth_info = depth_map.get_depth_info(&seg);
            // Add the block, with config info.
            elem_buff.push(ReflowElement::Block(Box::new(ReflowBlock::from_config(
                vec![seg],
                reflow_config.clone(),
                depth_info,
            ))));

            // Empty the buffer
            seg_buff.clear();
        }

        if !seg_buff.is_empty() {
            elem_buff.push(ReflowPoint::new(seg_buff).into());
        }

        elem_buff
    }

    pub fn from_around_target(
        target_segment: &Box<dyn Segment>,
        root_segment: Box<dyn Segment>,
        sides: &str,
    ) -> ReflowSequence {
        let all_raws = root_segment.get_raw_segments();
        let target_raws = target_segment.get_raw_segments();

        assert!(!target_raws.is_empty());

        let pre_idx = all_raws.iter().position(|x| x == &target_raws[0]).unwrap();
        let post_idx =
            all_raws.iter().position(|x| x == &target_raws[target_raws.len() - 1]).unwrap() + 1;

        let mut pre_idx = pre_idx;
        let mut post_idx = post_idx;

        if sides == "both" || sides == "before" {
            pre_idx -= 1;
            for i in (0..=pre_idx).rev() {
                if all_raws[i].is_code() {
                    pre_idx = i;
                    break;
                }
            }
        }

        if sides == "both" || sides == "after" {
            // for i in post_idx..all_raws.len() {
                for (i, _item) in all_raws.iter().enumerate().skip(post_idx){
                if all_raws[i].is_code() {
                    post_idx = i;
                    break;
                }
            }
            post_idx += 1;
        }

        let segments = &all_raws[pre_idx..post_idx];
        ReflowSequence::from_raw_segments(
            segments.to_vec(),
            root_segment,
            // FIXME:
            FluffConfig::default(),
            None,
        )
    }

    #[allow(unused_variables)]
    pub fn insert(
        self,
        insertion: Box<dyn Segment>,
        target: Box<dyn Segment>,
        pos: &'static str,
    ) -> Self {
        let target_idx = self.find_element_idx_with(&target);

        
        let default_value = Default::default();
        let new_block = ReflowBlock::from_config(vec![insertion.clone()], default_value, <_>::default());
        // let new_block = ReflowBlock::from_config(vec![insertion.clone()], todo!(), <_>::default());

        if pos == "before" {
            let mut new_elements = self.elements[..target_idx].to_vec();
            new_elements.push(new_block.into());
            new_elements.push(ReflowPoint::default().into());
            new_elements.extend_from_slice(&self.elements[target_idx..]);

            let new_lint_result = LintResult::new(
                target.clone().into(),
                vec![LintFix::create_before(target, vec![insertion])],
                None,
                None,
                None,
            );

            return ReflowSequence {
                root_segment: self.root_segment,
                elements: new_elements,
                lint_results: vec![new_lint_result],
            };
        }

        self
    }

    fn find_element_idx_with(&self, target: &Box<dyn Segment>) -> usize {
        self.elements
            .iter()
            .position(|elem| elem.segments().contains(target))
            .unwrap_or_else(|| panic!("Target [{:?}] not found in ReflowSequence.", target))
    }

    pub fn without(self, target: &Box<dyn Segment>) -> ReflowSequence {
        let removal_idx = self.find_element_idx_with(target);
        if removal_idx == 0 || removal_idx == self.elements.len() - 1 {
            panic!("Unexpected removal at one end of a ReflowSequence.");
        }
        if let ReflowElement::Point(_) = &self.elements[removal_idx] {
            panic!("Not expected removal of whitespace in ReflowSequence.");
        }
        let merged_point = ReflowPoint::new(
            [self.elements[removal_idx - 1].segments(), self.elements[removal_idx + 1].segments()]
                .concat(),
        );
        let mut new_elements = self.elements[..removal_idx - 1].to_vec();
        new_elements.push(ReflowElement::Point(merged_point));
        new_elements.extend_from_slice(&self.elements[removal_idx + 2..]);

        ReflowSequence {
            elements: new_elements,
            root_segment: self.root_segment.clone(),
            lint_results: vec![LintResult::new(
                target.clone().into(),
                vec![LintFix::delete(target.clone())],
                None,
                None,
                None,
            )],
        }
    }

    pub fn respace(mut self, strip_newlines: bool, filter: Filter) -> Self {
        let mut lint_results = take(&mut self.lint_results);
        let mut new_elements = Vec::new();

        for (point, pre, post) in self.iter_points_with_constraints() {
            let (new_lint_results, mut new_point) =
                point.respace_point(pre, post, lint_results.clone(), strip_newlines);

            let ignore = if new_point.segments.iter().any(|seg| seg.is_type("newline"))
                || post.as_ref().map_or(false, |p| p.class_types().contains("end_of_file"))
            {
                filter == Filter::Inline
            } else {
                filter == Filter::Newline
            };

            if ignore {
                new_point = point.clone();
            } else {
                lint_results = new_lint_results;
            }

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

    pub fn rebreak(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("rebreak cannot currently handle pre-existing embodied fixes");
        }

        // Delegate to the rebreak algorithm
        let (elem_buff, lint_results) = rebreak_sequence(self.elements, self.root_segment.clone());

        ReflowSequence { root_segment: self.root_segment, elements: elem_buff, lint_results }
    }

    pub fn reindent(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("reindent cannot currently handle pre-existing embodied fixes");
        }

        let single_indent = construct_single_indent("space", 4);

        let (elements, indent_results) =
            lint_indent_points(self.elements, &single_indent, <_>::default(), <_>::default());

        Self { root_segment: self.root_segment, elements, lint_results: indent_results }
    }

    pub fn break_long_lines(self) -> Self {
        if !self.lint_results.is_empty() {
            panic!("break_long_lines cannot currently handle pre-existing embodied fixes");
        }

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

    pub fn elements(&self) -> &[ReflowElement] {
        &self.elements
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Inline,
    Newline,
}
