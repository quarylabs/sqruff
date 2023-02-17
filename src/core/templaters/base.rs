use crate::core::config::FluffConfig;
use std::collections::HashMap;
use crate::core::config::FluffConfig;

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

/// A templated SQL file.
/// This is the response of a templaters .process() method
/// and contains both references to the original file and also
/// the capability to split up that file when lexing.
pub struct TemplatedFile {

}

/// A templater which does nothing.
/// This also acts as the base templating class.
pub struct RawTemplater {
}

impl Default for RawTemplater {
    /// Placeholder init function.
    /// Here we should load any initial config found in the root directory. The init
    /// function shouldn't take any arguments at this stage as we assume that it will
    /// load its own config. Maybe at this stage we might allow override parameters to
    /// be passed to the linter at runtime from the cli - that would be the only time we
    /// would pass arguments in here.
    fn default() -> Self {
        RawTemplater {}
    }
}

impl RawTemplater {
    fn get_name() -> &'static str {
        "raw"
    }
    fn get_templater_selector() -> &'static str {
        "templater"
    }
    /// Given files to be processed, return a valid processing sequence.
    fn sequence_files(&self, file_names: Vec<&str>, config: Option<FluffConfig>, formatter: None) -> impl Iterator<Item = &str> + '_ {
        file_names.into_iter()
    }
    /// Process a string and return a TemplatedFile.
    ///
    /// Note that the arguments are enforced as keywords
    /// because Templaters can have differences in their
    /// `process` method signature.
    /// A Templater that only supports reading from a file
    /// would need the following signature:
    /// process(*, fname, in_str=None, config=None)
    /// (arguments are swapped)
    ///
    /// * 'in_str' is the input string.
    /// * 'fname' is The filename of this string. This is mostly for loading config files at runtime.
    /// * 'config' is the specific config to use for this templating operation. Only necessary for some templaters.
    /// * 'formatter' is the Optional object for output.
    fn process(&self, _: &str, in_str: &str, file_name: &str, config: Option<FluffConfig>, formatter: Option<None> ) -> (Option<TemplatedFile>, Vec<RawFileSlice>) {

    }
}

#[cfg(test)]
mod tests_raw_templater {
    use super::*;

    #[test]
    fn test_RawTemplater() {
        t = RawTemplater{};
        instr = "SELECT * FROM {{blah}}";
        outstr = t.process(instr, "test");
        assert_eq!(outstr, instr);
    }
}

