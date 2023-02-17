use crate::core::config::FluffConfig;
use std::collections::HashMap;

/// Find the indices of all newlines in a string.
pub fn iter_indices_of_newlines(raw_str: &str) -> impl Iterator<Item = usize> + '_ {
    // TODO: This may be optimize-able by not doing it all up front.
    raw_str.match_indices('\n').map(|(idx, _)| idx).into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_indices_of_newlines() {
        vec![
            ("", vec![]),
            ("foo", vec![]),
            ("foo\nbar", vec![3]),
            ("\nfoo\n\nbar\nfoo\n\nbar\n", vec![0, 4, 5, 9, 13, 14, 18]),
        ]
        .into_iter()
        .for_each(|(in_str, expected)| {
            assert_eq!(
                expected,
                iter_indices_of_newlines(in_str).collect::<Vec<usize>>()
            )
        });
    }
}

/// A slice referring to a raw file.
#[derive(Debug, Clone)]
pub struct RawFileSlice {
    pub raw: &'static str,
    pub slice_type: &'static str,
    pub source_idx: isize,
    pub slice_subtype: Option<&'static str>,
    pub block_idx: isize,
}

impl Default for RawFileSlice {
    fn default() -> RawFileSlice {
        RawFileSlice {
            raw: "",
            slice_type: "",
            source_idx: 0,
            slice_subtype: None,
            block_idx: 0,
        }
    }
}

impl RawFileSlice {
    /// Return the closing index of this slice.
    pub fn end_source_idx(&self) -> isize {
        self.source_idx + self.raw.len() as isize
    }

    pub fn source_slice(&self) -> &'static str {
        panic!("Not implemented, this is supposed to be 'return slice(self.source_idx, self.end_source_idx())' in python.")
    }

    /// Based on its slice_type, does it only appear in the *source*?
    /// There are some slice types which are automatically source only.
    /// There are *also* some which are source only because they render to an empty string.
    pub fn is_source_only_slice(&self) -> bool {
        vec!["comment", "block_end", "block_start", "block_mid"].contains(&self.slice_type)
    }
}

/// A slice referring to a templated file.
#[derive(Debug, Clone)]
pub struct TemplatedFileSlice {
    pub slice_type: &'static str,
    // TODO Figure out what to do with slices
    // pub source_slice: slice::Slice,
    // pub templated_slice: slice::Slice,
}

/// Template-related info about the raw slices in a TemplateFile.
#[derive(Debug, Clone)]
pub struct RawSliceBlockInfo {
    /// Given a raw file slace, return its block ID. Useful for identifying
    /// regions of a file with respect to template control structures (for, if).
    block_ids: HashMap<RawFileSlice, isize>,

    /// List of block IDs that have the following characteristics:
    /// - Loop body
    /// - Containing only literals (no templating)
    literal_only_loops: Vec<isize>,
}
