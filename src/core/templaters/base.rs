use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::errors::{SQLFluffSkipFile, SQLFluffUserError, ValueError};
use std::ops::Range;

/// A slice referring to a templated file.
#[derive(Debug, Clone, PartialEq)]
struct TemplatedFileSlice {
    slice_type: String,
    source_slice: Range<usize>,
    pub templated_slice: Range<usize>,
}

impl TemplatedFileSlice {
    fn new(slice_type: &str, source_slice: Range<usize>, templated_slice: Range<usize>) -> Self {
        Self {
            slice_type: slice_type.to_string(),
            source_slice,
            templated_slice,
        }
    }
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
    pub templated_str: Option<String>,
    source_newlines: Vec<usize>,
    templated_newlines: Vec<usize>,
    raw_sliced: Vec<RawFileSlice>,
    sliced_file: Vec<TemplatedFileSlice>,
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
        // Assume that no sliced_file, means the file is not templated.
        let temp_str = templated_str.unwrap_or(source_str.clone());
        let (sliced_file, outer_raw_sliced): (Vec<TemplatedFileSlice>, Vec<RawFileSlice>) =
            match sliced_file {
                None => {
                    if temp_str != source_str {
                        panic!("Cannot instantiate a templated file unsliced!")
                    } else {
                        if raw_sliced.is_some() {
                            panic!("Templated file was not sliced, but not has raw slices.")
                        } else {
                            (
                                vec![TemplatedFileSlice::new(
                                    "literal",
                                    0..source_str.len(),
                                    0..source_str.len(),
                                )],
                                vec![RawFileSlice::new(
                                    source_str.clone(),
                                    "literal".to_string(),
                                    0,
                                    None,
                                    None,
                                )],
                            )
                        }
                    }
                }
                Some(sliced_file) => {
                    if let Some(raw_sliced) = raw_sliced {
                        (sliced_file, raw_sliced)
                    } else {
                        panic!("Templated file was sliced, but not raw.")
                    }
                }
            };

        // Precalculate newlines, character positions.
        let source_newlines: Vec<usize> = iter_indices_of_newlines(source_str.as_str()).collect();
        let templated_newlines: Vec<usize> = iter_indices_of_newlines(temp_str.as_str()).collect();

        // Consistency check raw string and slices.
        let mut pos = 0;
        for rfs in &outer_raw_sliced {
            if rfs.source_idx != pos {
                panic!(
                    "TemplatedFile. Consistency fail on running source length. {} != {}",
                    pos, rfs.source_idx
                )
            }
            pos += rfs.raw.len();
        }
        if pos != source_str.len() {
            panic!(
                "TemplatedFile. Consistency fail on final source length. {} != {}",
                pos,
                source_str.len()
            )
        }

        // Consistency check templated string and slices.
        let mut previous_slice: Option<&TemplatedFileSlice> = None;
        let mut outer_tfs: Option<&TemplatedFileSlice> = None;
        for tfs in &sliced_file {
            match &previous_slice {
                Some(previous_slice) => {
                    if tfs.templated_slice.start != previous_slice.templated_slice.end {
                        return Err(SQLFluffSkipFile::new(
                            "Templated slices found to be non-contiguous.".to_string(), // TODO Make this nicer again
                                                                                        // format!(
                                                                                        //     "Templated slices found to be non-contiguous. {:?} (starting {:?}) does not follow {:?} (starting {:?})",
                                                                                        //     tfs.templated_slice,
                                                                                        //     templated_str[tfs.templated_slice],
                                                                                        //     previous_slice.templated_slice,
                                                                                        //     templated_str[previous_slice.templated_slice],
                                                                                        // )
                        ));
                    }
                }
                None => {
                    if tfs.templated_slice.start != 0 {
                        return Err(SQLFluffSkipFile::new(format!(
                            "First templated slice does not start at 0, (found slice {:?})",
                            tfs.templated_slice
                        )));
                    }
                }
            }
            previous_slice = Some(tfs);
            outer_tfs = Some(&tfs)
        }
        if !sliced_file.is_empty() {
            if !temp_str.is_empty() {
                if let Some(outer_tfs) = outer_tfs {
                    if outer_tfs.templated_slice.end != temp_str.len() {
                        return Err(SQLFluffSkipFile::new(format!("Last templated slice does not end at end of string, (found slice {:?})", outer_tfs.templated_slice)));
                    }
                }
            }
        }

        Ok(TemplatedFile {
            raw_sliced: outer_raw_sliced,
            source_newlines,
            templated_newlines,
            source_str: source_str.clone(),
            sliced_file: sliced_file,
            f_name,
            templated_str: Some(temp_str),
        })
    }

    /// Return true if there's a templated file.
    pub fn is_templated(&self) -> bool {
        self.templated_str.is_some()
    }

    /// Get the line number and position of a point in the source file.
    /// Args:
    ///  - char_pos: The character position in the relevant file.
    ///  - source: Are we checking the source file (as opposed to the templated file)
    ///
    /// Returns: line_number, line_position
    pub fn get_line_pos_of_char_pos(&self, char_pos: usize, source: bool) -> (usize, usize) {
        let ref_str = if source {
            &self.source_newlines
        } else {
            &self.templated_newlines
        };
        match ref_str.binary_search(&char_pos) {
            Ok(nl_idx) | Err(nl_idx) => {
                if nl_idx > 0 {
                    ((nl_idx + 1), (char_pos - ref_str[nl_idx - 1]))
                } else {
                    // NB: line_pos is char_pos + 1 because character position is 0-indexed,
                    // but the line position is 1-indexed.
                    (1, (char_pos + 1))
                }
            }
        }
    }

    /// Create TemplatedFile from a string.
    pub fn from_string(raw: String) -> TemplatedFile {
        // TODO: Might need to deal with this unwrap
        TemplatedFile::new(raw.clone(), "<string>".to_string(), None, None, None, None).unwrap()
    }

    /// Get templated string
    pub fn get_templated_string(&self) -> Option<&str> {
        self.templated_str.as_ref().map(|s| s.as_str())
    }

    /// Return the templated file if coerced to string.
    pub fn to_string(&self) -> String {
        self.templated_str.clone().unwrap().to_string()
    }

    /// Return a list a slices which reference the parts only in the source.
    ///
    /// All of these slices should be expected to have zero-length in the templated file.
    ///
    ///         The results are NECESSARILY sorted.
    fn source_only_slices(&self) -> Vec<RawFileSlice> {
        let mut ret_buff = vec![];
        for element in &self.raw_sliced {
            if element.is_source_only_slice() {
                ret_buff.push(element.clone());
            }
        }
        ret_buff
    }

    /// Find a subset of the sliced file which touch this point.
    ///
    ///     The last_idx is exclusive, as the intent is to use this as a slice.
    pub fn find_slice_indices_of_templated_pos(
        &self,
        templated_pos: usize,
        start_idx: Option<usize>,
        inclusive: Option<bool>,
    ) -> Result<(usize, usize), ValueError> {
        let start_idx = start_idx.unwrap_or(0);
        let inclusive = inclusive.unwrap_or(true);

        let mut first_idx: Option<usize> = None;
        let mut last_idx = start_idx;

        // Work through the sliced file, starting at the start_idx if given
        // as an optimisation hint. The sliced_file is a list of TemplatedFileSlice
        // which reference parts of the templated file and where they exist in the
        // source.
        for (idx, elem) in self.sliced_file[start_idx..].iter().enumerate() {
            last_idx = idx + start_idx;
            if elem.templated_slice.end >= templated_pos {
                if first_idx.is_none() {
                    first_idx = Some(idx + start_idx);
                }
                if elem.templated_slice.end > templated_pos {
                    break;
                } else if !inclusive && elem.templated_slice.end >= templated_pos {
                    break;
                }
            }
        }

        // If we got to the end add another index
        if last_idx == self.sliced_file.len() - 1 {
            last_idx += 1;
        }

        match first_idx {
            Some(first_idx) => Ok((first_idx, last_idx)),
            None => Err(ValueError::new("Position Not Found".to_string())),
        }
    }
}

/// Find the indices of all newlines in a string.
pub fn iter_indices_of_newlines(raw_str: &str) -> impl Iterator<Item = usize> + '_ {
    // TODO: This may be optimize-able by not doing it all up front.
    raw_str.match_indices('\n').map(|(idx, _)| idx).into_iter()
}

#[derive(Debug, PartialEq, Clone)]
enum RawFileSliceType {
    Comment,
    BlockEnd,
    BlockStart,
    BlockMid,
}

/// A slice referring to a raw file.
#[derive(Debug, PartialEq, Clone)]
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
    fn new(
        raw: String,
        slice_type: String,
        source_idx: usize,
        slice_subtype: Option<RawFileSliceType>,
        block_idx: Option<usize>,
    ) -> Self {
        Self {
            raw,
            slice_type,
            source_idx,
            slice_subtype,
            block_idx: block_idx.unwrap_or(0),
        }
    }
}

impl RawFileSlice {
    /// Return the closing index of this slice.
    fn end_source_idx(&self) -> usize {
        return self.source_idx + self.raw.len();
    }

    /// Return the a slice object for this slice.
    fn source_slice(&self) -> Range<usize> {
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

    #[test]
    /// Test the raw templater
    fn test__templater_raw() {
        let templater = RawTemplater::default();
        let in_str = "SELECT * FROM {{blah}}";

        let outstr = templater.process(in_str, "test.sql", None, None).unwrap();

        assert_eq!(outstr.templated_str, Some(in_str.to_string()));
    }

    const SIMPLE_SOURCE_STR: &str = "01234\n6789{{foo}}fo\nbarss";
    const SIMPLE_TEMPLATED_STR: &str = "01234\n6789x\nfo\nbarfss";
    fn simple_sliced_file() -> Vec<TemplatedFileSlice> {
        vec![
            TemplatedFileSlice::new("literal", 0..10, 0..10),
            TemplatedFileSlice::new("templated", 10..17, 10..12),
            TemplatedFileSlice::new("literal", 17..25, 12..20),
        ]
    }
    fn simple_raw_sliced_file() -> [RawFileSlice; 3] {
        [
            RawFileSlice::new("x".repeat(10), "literal".to_string(), 0, None, None),
            RawFileSlice::new("x".repeat(7), "templated".to_string(), 10, None, None),
            RawFileSlice::new("x".repeat(8), "literal".to_string(), 17, None, None),
        ]
    }
    fn complex_sliced_file() -> Vec<TemplatedFileSlice> {
        vec![
            TemplatedFileSlice::new("literal", 0..13, 0..13),
            TemplatedFileSlice::new("comment", 13..29, 13..13),
            TemplatedFileSlice::new("literal", 29..44, 13..28),
            TemplatedFileSlice::new("block_start", 44..68, 28..28),
            TemplatedFileSlice::new("literal", 68..81, 28..41),
            TemplatedFileSlice::new("templated", 81..86, 41..42),
            TemplatedFileSlice::new("literal", 86..110, 42..66),
            TemplatedFileSlice::new("templated", 68..86, 66..76),
            TemplatedFileSlice::new("literal", 68..81, 76..89),
            TemplatedFileSlice::new("templated", 81..86, 89..90),
            TemplatedFileSlice::new("literal", 86..110, 90..114),
            TemplatedFileSlice::new("templated", 68..86, 114..125),
            TemplatedFileSlice::new("literal", 68..81, 125..138),
            TemplatedFileSlice::new("templated", 81..86, 138..139),
            TemplatedFileSlice::new("literal", 86..110, 139..163),
            TemplatedFileSlice::new("templated", 110..123, 163..166),
            TemplatedFileSlice::new("literal", 123..132, 166..175),
            TemplatedFileSlice::new("block_end", 132..144, 175..175),
            TemplatedFileSlice::new("literal", 144..155, 175..186),
            TemplatedFileSlice::new("block_start", 155..179, 186..186),
            TemplatedFileSlice::new("literal", 179..189, 186..196),
            TemplatedFileSlice::new("templated", 189..194, 196..197),
            TemplatedFileSlice::new("literal", 194..203, 197..206),
            TemplatedFileSlice::new("literal", 179..189, 206..216),
            TemplatedFileSlice::new("templated", 189..194, 216..217),
            TemplatedFileSlice::new("literal", 194..203, 217..226),
            TemplatedFileSlice::new("literal", 179..189, 226..236),
            TemplatedFileSlice::new("templated", 189..194, 236..237),
            TemplatedFileSlice::new("literal", 194..203, 237..246),
            TemplatedFileSlice::new("block_end", 203..215, 246..246),
            TemplatedFileSlice::new("literal", 215..230, 246..261),
        ]
    }
    fn complex_raw_sliced_file() -> Vec<RawFileSlice> {
        vec![
            RawFileSlice::new(
                "x".repeat(13).to_string(),
                "literal".to_string(),
                0,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(16).to_string(),
                "comment".to_string(),
                13,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(15).to_string(),
                "literal".to_string(),
                29,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(24).to_string(),
                "block_start".to_string(),
                44,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(13).to_string(),
                "literal".to_string(),
                68,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(5).to_string(),
                "templated".to_string(),
                81,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(24).to_string(),
                "literal".to_string(),
                86,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(13).to_string(),
                "templated".to_string(),
                110,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(9).to_string(),
                "literal".to_string(),
                123,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(12).to_string(),
                "block_end".to_string(),
                132,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(11).to_string(),
                "literal".to_string(),
                144,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(24).to_string(),
                "block_start".to_string(),
                155,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(10).to_string(),
                "literal".to_string(),
                179,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(5).to_string(),
                "templated".to_string(),
                189,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(9).to_string(),
                "literal".to_string(),
                194,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(12).to_string(),
                "block_end".to_string(),
                203,
                None,
                None,
            ),
            RawFileSlice::new(
                "x".repeat(15).to_string(),
                "literal".to_string(),
                215,
                None,
                None,
            ),
        ]
    }

    struct FileKwargs {
        f_name: String,
        source_str: String,
        templated_str: Option<String>,
        sliced_file: Vec<TemplatedFileSlice>,
        raw_sliced_file: Vec<RawFileSlice>,
    }

    fn simple_file_kwargs() -> FileKwargs {
        FileKwargs {
            f_name: "test.sql".to_string(),
            source_str: "01234\n6789{{foo}}fo\nbarss".to_string(),
            templated_str: Some("01234\n6789x\nfo\nbarss".to_string()),
            sliced_file: simple_sliced_file().to_vec(),
            raw_sliced_file: simple_raw_sliced_file().to_vec(),
        }
    }

    fn complex_file_kwargs() -> FileKwargs {
        FileKwargs {
            f_name: "test.sql".to_string(),
            source_str: complex_raw_sliced_file()
                .iter()
                .fold(String::new(), |acc, x| acc + &x.raw),
            templated_str: None,
            sliced_file: complex_sliced_file().to_vec(),
            raw_sliced_file: complex_raw_sliced_file().to_vec(),
        }
    }

    #[test]
    /// Test TemplatedFile.get_line_pos_of_char_pos.
    fn test__templated_file_get_line_pos_of_char_pos() {
        let tests = [
            (simple_file_kwargs(), 0, 1, 1),
            (simple_file_kwargs(), 20, 3, 1),
            (simple_file_kwargs(), 24, 3, 5),
        ];

        for test in tests {
            let kwargs = test.0;

            let tf = TemplatedFile::new(
                kwargs.source_str,
                kwargs.f_name,
                kwargs.templated_str,
                Some(kwargs.sliced_file),
                Some(kwargs.raw_sliced_file),
                None,
            )
            .unwrap();

            let (res_line_no, res_line_pos) = tf.get_line_pos_of_char_pos(test.1, true);

            assert_eq!(res_line_no, test.2);
            assert_eq!(res_line_pos, test.3);
        }
    }

    #[test]
    fn test__templated_file_find_slice_indices_of_templated_pos() {
        let tests = vec![
            // "templated_position,inclusive,file_slices,sliced_idx_start,sliced_idx_stop",
            // TODO Fix these
            // (100, true, complex_file_kwargs(), 10, 11),
            // (13, true, complex_file_kwargs(), 0, 3),
            // (28, true, complex_file_kwargs(), 2, 5),
            // # Check end slicing.
            (12, true, simple_file_kwargs(), 1, 3),
            (20, true, simple_file_kwargs(), 2, 3),
            // Check inclusivity
            // (13, false, complex_file_kwargs(), 0, 1),
        ];

        for test in tests {
            let args = test.2;

            let file = TemplatedFile::new(
                args.source_str,
                args.f_name,
                args.templated_str,
                Some(args.sliced_file),
                Some(args.raw_sliced_file),
                None,
            )
            .unwrap();

            let (res_start, res_stop) = file
                .find_slice_indices_of_templated_pos(test.0, None, Some(test.1))
                .unwrap();

            assert_eq!(res_start, test.3);
            assert_eq!(res_stop, test.4);
        }
    }
}
