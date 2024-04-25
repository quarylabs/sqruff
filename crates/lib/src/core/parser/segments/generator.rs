use std::rc::Rc;

use crate::core::dialects::base::Dialect;
use crate::core::parser::matchable::Matchable;

type Generator = fn(&Dialect) -> Rc<dyn Matchable>;

#[derive(Debug, Clone)]
pub struct SegmentGenerator {
    func: Generator,
}

impl SegmentGenerator {
    // Define a new function to create a new SegmentGenerator
    pub fn new(func: Generator) -> SegmentGenerator {
        SegmentGenerator { func }
    }

    // Implement the expand function
    pub fn expand(&self, dialect: &Dialect) -> Rc<dyn Matchable> {
        (self.func)(dialect)
    }
}
