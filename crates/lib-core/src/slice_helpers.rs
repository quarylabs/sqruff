use std::ops::Range;

pub fn zero_slice<T: Copy>(i: T) -> Range<T> {
    i..i
}

pub fn is_zero_slice(s: &Range<usize>) -> bool {
    s.start == s.end
}

pub fn offset_slice(start: usize, offset: usize) -> Range<usize> {
    start..start + offset
}
