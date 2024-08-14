use std::hash::BuildHasherDefault;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::parser::matchable::Matchable;

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, BuildHasherDefault<ahash::AHasher>>;
pub type IndexSet<V> = indexmap::IndexSet<V, BuildHasherDefault<ahash::AHasher>>;

pub trait ToMatchable: Matchable + Sized {
    fn to_matchable(self) -> Arc<dyn Matchable> {
        Arc::new(self)
    }
}

impl<T: Matchable> ToMatchable for T {}

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

pub(crate) fn next_cache_key() -> u64 {
    // The value 0 is reserved for NonCodeMatcher. This grammar matcher is somewhat
    // of a singleton, so we don't need a unique ID in the same way as other grammar
    // matchers.
    static ID: AtomicU64 = AtomicU64::new(1);

    ID.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1)).unwrap()
}
