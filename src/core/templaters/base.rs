use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::{SQLFluffSkipFile, SQLFluffUserError};
use std::ops::Range;

/// A slice referring to a templated file.
#[derive(Debug)]
struct TemplatedFileSlice {
    slice_type: String,
    source_slice: Range<usize>,
    templated_slice: Range<usize>,
}

/// A templated SQL file.
///
/// This is the response of a `templater`'s `.process()` method
/// and contains both references to the original file and also
/// the capability to split up that file when lexing.
#[derive(Debug, PartialEq, Clone)]
pub struct TemplatedFile {
    source_str: String,
    f_name: String,
    templated_str: Option<String>,
}

impl TemplatedFile {
    fn new(
        source_str: String,
        f_name: String,
        templated_str: Option<String>,
        sliced_file: Option<Vec<TemplatedFileSlice>>,
        raw_sliced: Option<Vec<RawFileSlice>>,
        check_consistency: bool,
    ) -> Result<TemplatedFile, SQLFluffSkipFile> {
        panic!("Not implemented")
        // self.source_str = source_str
        // # An empty string is still allowed as the templated string.
        // self.templated_str = source_str if templated_str is None else templated_str
        // # If no fname, we assume this is from a string or stdin.
        // self.fname = fname
    }
}

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

enum RawFileSliceType {
    Comment,
    BlockEnd,
    BlockStart,
    BlockMid,
}

/// A slice referring to a raw file.
pub struct RawFileSlice {
    /// Source string
    raw: String,
    slice_type: String,
    /// Offset from beginning of source string
    source_idx: usize,
    slice_subtype: Option<RawFileSliceType>,
    /// Block index, incremented on start or end block tags, e.g. "if", "for"
    block_idx: usize,
}

impl RawFileSlice {
    /// Return the closing index of this slice.
    fn end_source_idx(&self) -> usize {
        return self.source_idx + self.raw.len();
    }

    /// Return the a slice object for this slice.
    fn source_slice(&self) -> std::ops::Range<usize> {
        self.source_idx..self.end_source_idx()
    }

    /// Based on its slice_type, does it only appear in the *source*?
    /// There are some slice types which are automatically source only.
    /// There are *also* some which are source only because they render
    /// to an empty string.
    fn is_source_only_slice(&self) -> bool {
        // TODO: should any new logic go here?
        if let Some(t) = &self.slice_subtype {
            match t {
                RawFileSliceType::Comment => true,
                RawFileSliceType::BlockStart => true,
                RawFileSliceType::BlockEnd => true,
                RawFileSliceType::BlockMid => true,
                _ => false,
            }
        } else {
            return false;
        }
    }
}

pub struct RawTemplater {}

impl Default for RawTemplater {
    fn default() -> Self {
        Self {}
    }
}

impl Templater for RawTemplater {
    fn name(&self) -> &str {
        "raw"
    }

    fn template_selection(&self) -> &str {
        "templater"
    }

    fn config_pairs(&self) -> (String, String) {
        return ("templater".to_string(), self.name().to_string());
    }

    fn sequence_files(
        &self,
        f_names: Vec<String>,
        _: Option<&FluffConfig>,

        _: Option<&dyn Formatter>,
    ) -> Vec<String> {
        // Default is to process in the original order.
        return f_names;
    }

    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: Option<&FluffConfig>,
        formatter: Option<&dyn Formatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError> {
        panic!("not implemented")
        // TemplatedFile{

        // }
        // return Some(TemplatedFile::newin_str, fname=fname))
    }
}

pub trait Templater {
    /// The name of the templater.
    fn name(&self) -> &str;

    /// Template Selector
    fn template_selection(&self) -> &str;

    /// Returns info about the given templater for output by the cli.
    fn config_pairs(&self) -> (String, String);

    /// Given files to be processed, return a valid processing sequence.
    fn sequence_files(
        &self,
        f_names: Vec<String>,
        config: Option<&FluffConfig>,

        formatter: Option<&dyn Formatter>,
    ) -> Vec<String>;

    /// Process a string and return a TemplatedFile.
    fn process(
        &self,
        in_str: &str,
        f_name: &str,
        config: Option<&FluffConfig>,

        formatter: Option<&dyn Formatter>,
    ) -> Result<TemplatedFile, SQLFluffUserError>;
}
