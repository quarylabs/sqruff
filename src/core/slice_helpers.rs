use std::ops::Range;

pub fn zero_slice<T: Copy>(i: T) -> Range<T> {
    i..i
}

pub fn offset_slice(start: usize, offset: usize) -> std::ops::Range<usize> {
    start..start + offset
}
