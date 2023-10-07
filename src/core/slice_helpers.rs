use std::ops::Range;

pub fn zero_slice<T: Copy>(i: T) -> Range<T> {
    i..i
}
