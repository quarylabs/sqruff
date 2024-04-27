use std::cell::RefCell;
use std::rc::Rc;

use anymap::AnyMap;

use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::templaters::base::TemplatedFile;

/// Struct for holding the context passed to rule eval function
#[derive(Clone, Debug)]
pub struct RuleContext<'a> {
    // These don't change within a file.
    pub dialect: &'a Dialect,
    pub fix: bool,
    pub templated_file: Option<TemplatedFile>,
    pub path: Option<String>,
    pub config: Option<&'a FluffConfig>,

    // These change within a file.
    /// segment: The segment in question
    pub segment: ErasedSegment,
    /// parent_stack: A tuple of the path from the root to this segment.
    pub parent_stack: Vec<ErasedSegment>,
    /// raw_stack: All of the raw segments so far in the file
    pub raw_stack: Vec<ErasedSegment>,
    /// memory: Arbitrary storage for the rule
    pub memory: Rc<RefCell<AnyMap>>,
    /// segment_idx: The index of this segment in the parent
    pub segment_idx: usize,
}

impl RuleContext<'_> {
    pub fn siblings_post(&self) -> Vec<ErasedSegment> {
        if !self.parent_stack.is_empty() {
            self.parent_stack.last().unwrap().segments()[self.segment_idx + 1..].to_vec()
        } else {
            Vec::new()
        }
    }
}
