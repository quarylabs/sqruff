use crate::parser::matchable::{Matchable, MatchableTraitImpl};

pub trait ToMatchable: Sized {
    fn to_matchable(self) -> Matchable;
}

impl<T: Into<MatchableTraitImpl>> ToMatchable for T {
    fn to_matchable(self) -> Matchable {
        Matchable::new(self.into())
    }
}
