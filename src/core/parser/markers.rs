use crate::core::templaters::base::TemplatedFile;
use std::cmp::{Ord, PartialEq, PartialOrd};
use std::ops::Range;

/// A reference to a position in a file.
///
/// Things to note:
/// - This combines the previous functionality of FilePositionMarker
///   and EnrichedFilePositionMarker. Additionally it contains a reference
///   to the original templated file.
/// - It no longer explicitly stores a line number or line position in the
///   source or template. This is extrapolated from the templated file as required.
/// - Positions in the source and template are with slices and therefore identify
///   ranges.
/// - Positions within the fixed file are identified with a line number and line
///   position, which identify a point.
/// - Arithmetic comparisons are on the location in the fixed file.
pub struct PositionMarker {
    pub source_slice: Range<usize>,
    pub templated_slice: Range<usize>,
    pub templated_file: TemplatedFile,
    pub working_line_no: usize,
    pub working_line_pos: usize,
}

impl PositionMarker {
    /// creates a PositionMarker
    ///
    /// Unlike the Python version, post_init is embedded into the new function.
    pub fn new(
        source_slice: Range<usize>,
        templated_slice: Range<usize>,
        templated_file: TemplatedFile,
        working_line_no: Option<usize>,
        working_line_pos: Option<usize>,
    ) -> Self {
        if working_line_no.is_none() || working_line_pos.is_none() {
            let (working_line_no, working_line_pos) =
                templated_file.get_line_pos_of_char_pos(templated_slice.start, false);
            PositionMarker {
                source_slice,
                templated_slice,
                templated_file,
                working_line_no,
                working_line_pos,
            }
        } else {
            PositionMarker {
                source_slice,
                templated_slice,
                templated_file,
                working_line_no: working_line_no.unwrap(),
                working_line_pos: working_line_pos.unwrap(),
            }
        }
    }

    /// Return the line and position of this marker in the source.
    pub fn templated_position(&self) -> (usize, usize) {
        self.templated_file
            .get_line_pos_of_char_pos(self.templated_slice.start, false)
    }

    /// Using the raw string provided to infer the position of the next.
    /// **Line position in 1-indexed.**
    pub fn infer_next_position(raw: &str, line_no: usize, line_pos: usize) -> (usize, usize) {
        if raw.is_empty() {
            return (line_no, line_pos);
        }
        let split: Vec<&str> = raw.split('\n').collect();
        (
            line_no + (split.len() - 1),
            if split.len() == 1 {
                line_pos + raw.len()
            } else {
                split.last().unwrap().len() + 1
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::core::parser::markers::PositionMarker;
    use std::ops::Range;

    #[test]
    fn test_markers__infer_next_position() {
        struct test {
            raw: String,
            start: Range<usize>,
            end: Range<usize>,
        }

        let tests: Vec<test> = vec![
            test {
                raw: "fsaljk".to_string(),
                start: (0..0),
                end: (0..6),
            },
            test {
                raw: "".to_string(),
                start: (2..2),
                end: (2..2),
            },
            test {
                raw: "\n".to_string(),
                start: (2..2),
                end: (3..1),
            },
            test {
                raw: "boo\n".to_string(),
                start: (2..2),
                end: (3..1),
            },
            test {
                raw: "boo\nfoo".to_string(),
                start: (2..2),
                end: (3..4),
            },
            test {
                raw: "\nfoo".to_string(),
                start: (2..2),
                end: (3..4),
            },
        ];

        for t in tests {
            assert_eq!(
                (t.end.start, t.end.end),
                PositionMarker::infer_next_position(&t.raw, t.start.start, t.start.end)
            );
        }
    }
}
