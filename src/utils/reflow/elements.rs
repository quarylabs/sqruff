use crate::core::parser::segments::raw::RawSegment;
use crate::utils::reflow::config::ReflowConfig;
use crate::utils::reflow::depth_map::DepthInfo;
use std::collections::HashSet;

/// A helper function to extract possible consumed whitespace.
///     Args:
///         segment (:obj:`RawSegment`, optional): A segment to test for
///             suitability and extract the source representation of if
///             appropriate. If passed None, then returns None.
///     Returns:
///         Returns the :code:`source_str` if the segment is of type
///         :code:`placeholder` and has a :code:`block_type` of
///         :code:`literal`. Otherwise None.
pub fn get_consumed_whitespace(segment: Option<RawSegment>) -> Option<String> {
    match segment {
        None => None,
        Some(segment) => {
            if !segment.is_type("placeholder") {
                return None;
            }
            let placeholder = cast(TemplateSegment, segment);
            if placeho_holder.blog_type != "literal" {
                return None;
            }
            return Some(placeholder.source_str);
        }
    }
}

/// Base reflow element class.
pub struct ReflowElement {
    segments: Vec<RawSegment>,
}

impl ReflowElement {
    pub fn new(segments: Vec<RawSegment>) -> ReflowElement {
        ReflowElement { segments }
    }

    fn _class_types(segments: Vec<RawSegment>) -> HashSet<String> {
        return segments
            .iter()
            .map(|segment| segment.class_type.clone())
            .collect();
    }

    /// Get the set of contained class types.
    /// Parallel to `BaseSegment.class_types`
    pub fn class_types(self: &Self) -> HashSet<String> {
        return ReflowElement::_class_types(self.segments.clone());
    }

    /// Get the current raw representation.
    pub fn raw(self: &Self) -> String {
        self.segments.clone().join("")
    }

    /// Return the number of newlines in this element.
    /// These newlines are either newline segments or contained
    /// within consumed sections of whitespace. This counts
    /// both.
    pub fn num_newlines(self: &Self) -> usize {
        self.segments
            .clone()
            .iter()
            .filter(|segment| segment.is_type("newline"))
            .count()
            + self
                .segments
                .clone()
                .iter()
                .filter(|segment| segment.is_type("whitespace"))
                .count("\n")
    }
}

pub struct ReflowBlock {
    element: ReflowElement,
}

impl ReflowBlock {
    pub fn from_config(
        segments: Vec<RawSegment>,
        config: ReflowConfig,
        depth_info: DepthInfo,
    ) -> ReflowBlock {
        panic!("Not implemented yet");
    }
}
