use std::ops::{Deref, DerefMut};

use crate::core::parser::matchable::Matchable;

use super::anyof::{one_of, AnyNumberOf};

/// Match an arbitrary number of elements separated by a delimiter.
///
/// Note that if there are multiple elements passed in that they will be treated
/// as different options of what can be delimited, rather than a sequence.
pub struct Delimited {
    base: AnyNumberOf,
}

impl Delimited {
    pub fn new(elements: Vec<Box<dyn Matchable>>) -> Self {
        Self {
            base: one_of(elements),
        }
    }
}

impl Deref for Delimited {
    type Target = AnyNumberOf;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Delimited {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
