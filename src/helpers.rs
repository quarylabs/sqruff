use crate::core::parser::matchable::Matchable;

pub trait ToMatchable: Matchable + Sized {
    fn to_matchable(self) -> Box<dyn Matchable> {
        Box::new(self) as Box<dyn Matchable>
    }
}

impl<T: Matchable> ToMatchable for T {}
