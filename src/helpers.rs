use crate::core::parser::matchable::Matchable;

pub trait ToMatchable: Matchable + Sized {
    fn to_matchable(self) -> Box<dyn Matchable> {
        self.boxed()
    }
}

impl<T: Matchable> ToMatchable for T {}

pub trait Boxed {
    fn boxed(self) -> Box<Self>
    where
        Self: Sized;
}

impl<T> Boxed for T {
    fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

pub fn capitalize(s: &str) -> String {
    assert!(s.is_ascii());

    let mut chars = s.chars();
    let Some(first_char) = chars.next() else {
        return String::new();
    };

    first_char
        .to_uppercase()
        .chain(chars.map(|ch| ch.to_ascii_lowercase()))
        .collect()
}

pub trait Config: Sized {
    fn config(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

impl<T> Config for T {}
