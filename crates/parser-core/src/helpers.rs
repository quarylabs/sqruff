use std::cell::RefCell;
use std::hash::BuildHasherDefault;
use std::panic;
use std::path::{Component, Path, PathBuf};
use std::sync::Once;

use crate::parser::matchable::{Matchable, MatchableTraitImpl};

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, BuildHasherDefault<ahash::AHasher>>;
pub type IndexSet<V> = indexmap::IndexSet<V, BuildHasherDefault<ahash::AHasher>>;

pub trait ToMatchable: Sized {
    fn to_matchable(self) -> Matchable;
}

impl<T: Into<MatchableTraitImpl>> ToMatchable for T {
    fn to_matchable(self) -> Matchable {
        Matchable::new(self.into())
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
                let top = stack.last().copied();

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

pub fn enter_panic(context: String) -> PanicContext {
    static ONCE: Once = Once::new();
    ONCE.call_once(PanicContext::init);

    with_ctx(|ctx| ctx.push(context));
    PanicContext { _priv: () }
}

#[must_use]
pub struct PanicContext {
    _priv: (),
}

impl PanicContext {
    #[allow(clippy::print_stderr)]
    fn init() {
        let default_hook = panic::take_hook();
        let hook = move |panic_info: &panic::PanicHookInfo<'_>| {
            with_ctx(|ctx| {
                if !ctx.is_empty() {
                    eprintln!("Panic context:");
                    for frame in ctx.iter() {
                        eprintln!("> {frame}\n");
                    }
                }
                default_hook(panic_info);
            });
        };
        panic::set_hook(Box::new(hook));
    }
}

impl Drop for PanicContext {
    fn drop(&mut self) {
        with_ctx(|ctx| assert!(ctx.pop().is_some()));
    }
}

fn with_ctx(f: impl FnOnce(&mut Vec<String>)) {
    thread_local! {
        static CTX: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }
    CTX.with(|ctx| f(&mut ctx.borrow_mut()));
}
