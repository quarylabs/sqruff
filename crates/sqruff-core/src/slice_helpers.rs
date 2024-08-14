use std::hash::BuildHasherDefault;
use std::ops::Range;

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, BuildHasherDefault<ahash::AHasher>>;
pub type IndexSet<V> = indexmap::IndexSet<V, BuildHasherDefault<ahash::AHasher>>;

pub fn zero_slice<T: Copy>(i: T) -> Range<T> {
    i..i
}

pub fn is_zero_slice(s: Range<usize>) -> bool {
    s.start == s.end
}

pub fn offset_slice(start: usize, offset: usize) -> Range<usize> {
    start..start + offset
}
