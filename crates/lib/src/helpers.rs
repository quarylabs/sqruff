use std::iter::once;
use std::path::{Component, Path, PathBuf};

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

pub fn skip_last<T>(mut iter: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    let last = iter.next();
    iter.scan(last, |state, item| std::mem::replace(state, Some(item)))
}

// https://github.com/rust-lang/rfcs/issues/2208#issuecomment-342679694
pub fn normalize(p: &Path) -> PathBuf {
    let mut stack: Vec<Component> = vec![];

    // We assume .components() removes redundant consecutive path separators.
    // Note that .components() also does some normalization of '.' on its own
    // anyways. This '.' normalization happens to be compatible with the
    // approach below.
    for component in p.components() {
        match component {
            // Drop CurDir components, do not even push onto the stack.
            Component::CurDir => {}

            // For ParentDir components, we need to use the contents of the stack.
            Component::ParentDir => {
                // Look at the top element of stack, if any.
                let top = stack.last().cloned();

                match top {
                    // A component is on the stack, need more pattern matching.
                    Some(c) => {
                        match c {
                            // Push the ParentDir on the stack.
                            Component::Prefix(_) => {
                                stack.push(component);
                            }

                            // The parent of a RootDir is itself, so drop the ParentDir (no-op).
                            Component::RootDir => {}

                            // A CurDir should never be found on the stack, since they are dropped
                            // when seen.
                            Component::CurDir => {
                                unreachable!();
                            }

                            // If a ParentDir is found, it must be due to it piling up at the start
                            // of a path. Push the new ParentDir onto
                            // the stack.
                            Component::ParentDir => {
                                stack.push(component);
                            }

                            // If a Normal is found, pop it off.
                            Component::Normal(_) => {
                                let _ = stack.pop();
                            }
                        }
                    }

                    // Stack is empty, so path is empty, just push.
                    None => {
                        stack.push(component);
                    }
                }
            }

            // All others, simply push onto the stack.
            _ => {
                stack.push(component);
            }
        }
    }

    // If an empty PathBuf would be return, instead return CurDir ('.').
    if stack.is_empty() {
        return PathBuf::from(".");
    }

    let mut norm_path = PathBuf::new();

    for item in &stack {
        norm_path.push(item);
    }

    norm_path
}
