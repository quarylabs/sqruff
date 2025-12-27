use std::ops::Range;

use crate::templaters::{TemplatedFile, TemplatedFileSlice};

pub use sqruff_parser_core::parser::lexer::{
    Cursor, Element, LexSource, Lexer, Match, Matcher, Pattern, RawSource, SearchPatternKind,
};

impl LexSource for TemplatedFile {
    type Slice = TemplatedFileSlice;

    fn templated_str(&self) -> &str {
        self.templated()
    }

    fn slices(&self) -> &[Self::Slice] {
        &self.sliced_file
    }

    fn slice_type<'a>(&self, slice: &'a Self::Slice) -> &'a str {
        slice.slice_type.as_str()
    }

    fn source_range(&self, slice: &Self::Slice) -> Range<usize> {
        slice.source_slice.clone()
    }

    fn templated_range(&self, slice: &Self::Slice) -> Range<usize> {
        slice.templated_slice.clone()
    }
}
