use std::collections::HashMap;

/// Struct for holding the context passed to rule eval function
pub struct RuleContext {
    // // These don't change within a file.
    // dialect: Dialect,
    // fix: bool,
    // templated_file: Option<TemplatedFile>,
    // path: Option<String>,
    // config: FluffConfig,
    //
    // // These change within a file.
    // /// segment: The segment in question
    // segment: BaseSegment,
    // /// parent_stack: A tuple of the path from the root to this segment.
    // parent_stack: Vec<BaseSegment>,
    // /// raw_stack: All of the raw segments so far in the file
    // raw_stack: Vector<RawSegment>,
    // /// memory: Arbitrary storage for the rule
    // memory: HashMap<String,String>,
    // /// segment_idx: The index of this segment in the parent
    // segment_idx: int,
}

// impl Default for RuleContext {
//     fn default() -> Self {
//         Self {
//             dialect: Dialect::ansi(),
//             fix: false,
//             templated_file: None,
//             path: None,
//             config: FluffConfig::default(),
//             segment: BaseSegment::default(),
//             parent_stack: tuple(),
//             raw_stack: tuple(),
//             memory: dict(),
//             segment_idx: 0,
//         }
//     }
// }
