use std::sync::Arc;

use crate::dialects::base::Dialect;
use crate::parser::matchable::Matchable;

type Generator = fn(&Dialect) -> Arc<dyn Matchable>;

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
    pub fn expand(&self, dialect: &Dialect) -> Arc<dyn Matchable> {
        (self.func)(dialect)
    }
}
