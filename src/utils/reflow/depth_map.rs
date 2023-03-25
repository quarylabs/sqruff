use crate::core::parser::segments::base::PathStep;

/// An element of the stack_positions property of DepthInfo.
#[derive(Debug, PartialEq, Eq, Clone)]
struct StackPosition {
    idx: usize,
    len: usize,
    type_: String,
}

impl StackPosition {
    /// Interpret a path step for stack_positions.
    fn stack_pos_interpreter(path_step: &PathStep) -> String {
        if path_step.idx == 0 && path_step.idx == path_step.len - 1 {
            "solo".to_string()
        } else if path_step.idx == 0 {
            "start".to_string()
        } else if path_step.idx == path_step.len - 1 {
            "end".to_string()
        } else {
            "".to_string()
        }
    }

    /// Interpret a PathStep to construct a StackPosition
    fn from_path_step(path_step: &PathStep) -> StackPosition {
        StackPosition {
            idx: path_step.idx,
            len: path_step.len,
            type_: StackPosition::stack_pos_interpreter(path_step),
        }
    }
}
