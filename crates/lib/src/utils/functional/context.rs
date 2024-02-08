use super::segments::Segments;
use crate::core::rules::context::RuleContext;

pub struct FunctionalContext {
    context: RuleContext,
}

impl FunctionalContext {
    pub fn new(context: RuleContext) -> FunctionalContext {
        FunctionalContext { context }
    }

    pub fn segment(&self) -> Segments {
        Segments::new(self.context.segment.clone(), self.context.templated_file.clone())
    }

    // pub fn parent_stack(&self) -> Segments {
    //     // Assuming `Segments::from_slice` is a method to create Segments from a
    // slice     Segments::from_slice(&self.context.parent_stack,
    // self.context.templated_file) }

    // pub fn siblings_pre(&self) -> Segments {
    //     Segments::from_slice(&self.context.siblings_pre,
    // self.context.templated_file) }

    pub fn siblings_post(&self) -> Segments {
        Segments::from_vec(self.context.siblings_post(), self.context.templated_file.clone())
    }

    // pub fn raw_stack(&self) -> Segments {
    //     Segments::from_slice(&self.context.raw_stack,
    // self.context.templated_file) }

    // pub fn raw_segments(&self) -> Segments {
    //     let file_segment = &self.context.parent_stack[0];
    //     // Assuming `get_raw_segments` returns a slice or Vec
    //     Segments::from_slice(file_segment.get_raw_segments(),
    // self.context.templated_file) }
}
