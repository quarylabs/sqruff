use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
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
pub struct TemplatedFile;

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

struct RawTemplater {}

impl Templater for RawTemplater {
    fn name(&self) -> &str {
        "raw"
    }

    fn template_selection(&self) -> &str {
        "templater"
    }

    fn config_pairs(&self) -> (String, String) {
        return ("templater".to_string(), self.name().to_string())
    }

    fn sequence_files(&self, f_names: Vec<String>, _: Option<FluffConfig>, _: Option<dyn Formatter>) -> Vec<String>{
        // Default is to process in the original order.
        return f_names
    }

    fn process(&self, in_str: str, f_name: str, config: OPtion<FluffConfig>, formatter: Option<dyn Formatter>) -> (Option<TemplatedFile>) {
        panic!("Not implemented yet.")
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
    fn sequence_files(&self, f_names: Vec<String>, config: Option<FluffConfig>, formatter: Option<dyn Formatter>) -> Vec<String>

    /// Process a string and return a TemplatedFile.
    fn process(&self, in_str: str, f_name: str, config: OPtion<FluffConfig>, formatter: Option<dyn Formatter>) -> (Option<TemplatedFile>);
}