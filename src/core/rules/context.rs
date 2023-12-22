use std::collections::HashMap;

use crate::core::config::FluffConfig;
use crate::core::dialects::base::Dialect;
use crate::core::parser::segments::base::Segment;
use crate::core::templaters::base::TemplatedFile;

/// Struct for holding the context passed to rule eval function
pub struct RuleContext {
    // These don't change within a file.
    dialect: Dialect,
    fix: bool,
    templated_file: Option<TemplatedFile>,
    path: Option<String>,
    config: FluffConfig,

    // These change within a file.
    /// segment: The segment in question
    segment: Box<dyn Segment>,
    /// parent_stack: A tuple of the path from the root to this segment.
    parent_stack: Vec<Box<dyn Segment>>,
    /// raw_stack: All of the raw segments so far in the file
    raw_stack: Vec<Box<dyn Segment>>,
    /// memory: Arbitrary storage for the rule
    memory: HashMap<String, String>,
    /// segment_idx: The index of this segment in the parent
    segment_idx: usize,
}
