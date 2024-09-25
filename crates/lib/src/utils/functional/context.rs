use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::rules::context::RuleContext;

pub struct FunctionalContext<'a> {
    context: RuleContext<'a>,
}

impl<'a> FunctionalContext<'a> {
    pub fn new(context: RuleContext<'a>) -> FunctionalContext {
        FunctionalContext { context }
    }

    pub fn segment(&self) -> Segments {
        Segments::new(
            self.context.segment.clone(),
            self.context.templated_file.clone(),
        )
    }

    pub fn siblings_post(&self) -> Segments {
        Segments::from_vec(
            self.context.siblings_post(),
            self.context.templated_file.clone(),
        )
    }

    pub fn parent_stack(&self) -> Segments {
        Segments::from_vec(
            self.context.parent_stack.clone(),
            self.context.templated_file.clone(),
        )
    }
}
