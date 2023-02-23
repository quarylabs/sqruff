use crate::core::config::FluffConfig;
use crate::core::parser::segments::base::BaseSegment;
use crate::core::parser::segments::raw::RawSegment;
use crate::core::rules::base::LintResult;
use crate::utils::reflow::config::ReflowConfig;
use crate::utils::reflow::depth_map::DepthMap;

#[derive(Debug, Clone)]
pub struct ReflowSequence {
    elements: ReflowSequenceType,
    root_segments: BaseSegment,
    reflow_config: ReflowConfig,
    depth_map: Option<DepthMap>,
    /// This keeps track of fixes generated in the chaining process.
    /// Alternatively pictured: This is the list of fixes required
    /// to generate this sequence. We can build on this as we edit
    /// the sequence.
    /// Rather than saving *fixes* directly, we package them into
    /// LintResult objects to make it a little easier to expose them
    /// in the CLI.
    lint_results: Vec<LintResult>,
}

impl ReflowSequence {
    pub fn new(
        elements: ReflowSequenceType,
        root_segments: BaseSegment,
        reflow_config: ReflowConfig,
        depth_map: Option<DepthMap>,
        lint_results: Option<Vec<LintResult>>,
    ) -> ReflowSequence {
        return ReflowSequence {
            elements,
            root_segments,
            reflow_config,
            depth_map,
            lint_results: lint_results.unwrap_or(vec![]),
        };
    }

    // pub fn _elements_from_raw_segments(segments: Vec<RawSegment>, reflow_config: ReflowConfig, depth_map: DepthMap) -> ReflowSequenceType {
    //     panic!("Not implemented yet")
    // }

    pub fn from_raw_segments(
        segments: Vec<RawSegment>,
        root_segment: BaseSegment,
        config: FluffConfig,
        depth_map: Option<DepthMap>,
    ) -> ReflowSequence {
        let reflow_config = ReflowConfig::from_fluff_config(config);
        let replaced_depth_map =
            depth_map.unwrap_or(DepthMap::from_raws_roots(segments, root_segment));
        ReflowSequence::new(
            ReflowSequence::_elements_from_raw_segments(
                segments.clone(),
                reflow_config,
                replaced_depth_map.clone(),
            ),
            root_segment.clone(),
            reflow_config.clone(),
            Some(replaced_depth_map),
            None,
        )
    }
}
