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

    first_char.to_uppercase().chain(chars.map(|ch| ch.to_ascii_lowercase())).collect()
}

pub trait Config: Sized {
    fn config(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

impl<T> Config for T {}

#[derive(Clone, Debug)]
pub struct HashableFancyRegex(pub fancy_regex::Regex);

impl std::ops::DerefMut for HashableFancyRegex {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for HashableFancyRegex {
    type Target = fancy_regex::Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Eq for HashableFancyRegex {}

impl PartialEq for HashableFancyRegex {
    fn eq(&self, other: &HashableFancyRegex) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl std::hash::Hash for HashableFancyRegex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}
