use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::{SQLFluffSkipFile, SQLFluffUserError};
use std::ops::Range;

/// A slice referring to a templated file.
#[derive(Debug, Clone)]
struct TemplatedFileSlice {
    slice_type: String,
    source_slice: Range<usize>,
    pub templated_slice: Range<usize>,
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
    /// Initialise the TemplatedFile.
    /// If no templated_str is provided then we assume that
    /// the file is NOT templated and that the templated view
    /// is the same as the source view.
    fn new(
        source_str: String,
        f_name: String,
        templated_str: Option<String>,
        sliced_file: Option<Vec<TemplatedFileSlice>>,
        raw_sliced: Option<Vec<RawFileSlice>>,
        check_consistency: Option<bool>,
    ) -> Result<TemplatedFile, SQLFluffSkipFile> {
        let temp_str = templated_str.unwrap_or(source_str.clone());
        if sliced_file.is_none() && temp_str != source_str {
            // TODO: This is a bit of a hack. We should probably have a clean error be returned here.
            panic!("Cannot instantiate a templated file unsliced!")
        };
        // self.source_str = source_str
        // # An empty string is still allowed as the templated string.
        // self.templated_str = source_str if templated_str is None else templated_str
        // # If no fname, we assume this is from a string or stdin.
        // self.fname = fname

        //  NOTE: The "check_consistency" flag should always be True when using
        // SQLFluff in real life. This flag was only added because some legacy
        // templater tests in test/core/templaters/jinja_test.py use hardcoded
        // test data with issues that will trigger errors here. It would be cool
        // to fix that data someday. I (Barry H.) started looking into it, but
        // it was much trickier than I expected, because bits of the same data
        // are shared across multiple tests.
        if check_consistency.unwrap_or(true) {
            // Sanity check raw string and slices.
            let mut pos = 0;
            let rfs: Option<RawFileSlice> = None;
            raw_sliced.map(|raw_sliced| {
                for (idx, rs) in raw_sliced.iter().enumerate() {
                    if rs.source_idx != pos {
                        panic!("Raw slices found to be non-contiguous.")
                    }
                    pos += rs.raw.len();
                }
                if pos != source_str.len() {
                    panic!("Raw slices found to be non-contiguous.")
                }
            });

            // Sanity check templated string and slices.
            // # Sanity check templated string and slices.
            let mut previous_slice: Option<TemplatedFileSlice> = None;
            let mut tfs: Option<&TemplatedFileSlice> = None;
            sliced_file.map(|sliced_file| {
                for (idx, tfs) in sliced_file.iter().enumerate() {
                    if let Some(ps) = previous_slice {
                        if tfs.templated_slice.start != ps.templated_slice.end {
                            //     raise SQLFluffSkipFile(  # pragma: no cover
                            //                              "Templated slices found to be non-contiguous. "
                            //                              f"{tfs.templated_slice} (starting"
                            //                              f" {self.templated_str[tfs.templated_slice]!r})"
                            //                              f" does not follow {previous_slice.templated_slice} "
                            //                              "(starting "
                            //                              f"{self.templated_str[previous_slice.templated_slice]!r}"
                            //                              ")"

                            panic!("Templated slices found to be non-contiguous.")
                        } else {
                            if tfs.templated_slice.start != 0 {
                                //     raise SQLFluffSkipFile(  # pragma: no cover
                                //                              "First Templated slice not started at index 0 "
                                //                              f"(found slice {tfs.templated_slice})"
                                // )
                                panic!("First Templated slice not started at index 0")
                            }
                        }
                        if sliced_file.len() > 0 && temp_str.len() > 0 {
                            if tfs.templated_slice.end != temp_str.len() {
                                // raise SQLFluffSkipFile(  # pragma: no cover
                                //                          "Length of templated file mismatch with final slice: "
                                //                          f"{len(templated_str)} != {tfs.templated_slice.stop}."
                                panic!("Length of templated file mismatch with final slice.")
                            }
                        }
                        previous_slice = Some(tfs.clone());
                    }
                }
            });
        };

        Ok(TemplatedFile {
            source_str,
            f_name,
            templated_str: Some(temp_str),
        })
    }

    /// Return true if there's a templated file.
    pub fn is_templated(&self) -> bool {
        self.templated_str.is_some()
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
    pub source_idx: usize,
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
        if let Ok(tf) = TemplatedFile::new(
            in_str.to_string(),
            f_name.to_string(),
            None,
            None,
            None,
            None,
        ) {
            return Ok(tf);
        }
        panic!("Not implemented")
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
