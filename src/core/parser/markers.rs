use crate::core::slice_helpers::zero_slice;
use crate::core::templaters::base::TemplatedFile;
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
#[derive(Debug, Clone)]
pub struct PositionMarker {
    pub source_slice: Range<usize>,
    pub templated_slice: Range<usize>,
    pub templated_file: TemplatedFile,
    pub working_line_no: usize,
    pub working_line_pos: usize,
}

impl Default for PositionMarker {
    fn default() -> Self {
        PositionMarker {
            source_slice: (0..0),
            templated_slice: (0..0),
            templated_file: TemplatedFile::from_string("".to_string()),
            working_line_no: 0,
            working_line_pos: 0,
        }
    }
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

    /// Location tuple for the working position.
    pub fn working_loc(&self) -> (usize, usize) {
        (self.working_line_no, self.working_line_pos)
    }

    /// Convenience method for creating point markers.
    pub fn from_point(
        source_point: usize,
        templated_point: usize,
        templated_file: TemplatedFile,
        working_line_no: Option<usize>,
        working_line_pos: Option<usize>,
    ) -> Self {
        return Self::new(
            zero_slice(source_point),
            zero_slice(templated_point),
            templated_file,
            working_line_no,
            working_line_pos,
        );
    }
}

impl PartialEq for PositionMarker {
    fn eq(&self, other: &Self) -> bool {
        self.working_loc() == other.working_loc()
    }
}

impl PartialOrd for PositionMarker {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.working_loc().cmp(&other.working_loc()))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::parser::markers::PositionMarker;
    use crate::core::templaters::base::TemplatedFile;
    use std::ops::Range;

    /// Test that we can correctly infer positions from strings.
    #[test]
    fn test_markers__infer_next_position() {
        struct Test {
            raw: String,
            start: Range<usize>,
            end: Range<usize>,
        }

        let tests: Vec<Test> = vec![
            Test {
                raw: "fsaljk".to_string(),
                start: (0..0),
                end: (0..6),
            },
            Test {
                raw: "".to_string(),
                start: (2..2),
                end: (2..2),
            },
            Test {
                raw: "\n".to_string(),
                start: (2..2),
                end: (3..1),
            },
            Test {
                raw: "boo\n".to_string(),
                start: (2..2),
                end: (3..1),
            },
            Test {
                raw: "boo\nfoo".to_string(),
                start: (2..2),
                end: (3..4),
            },
            Test {
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

    /// Test that we can correctly infer positions from strings & locations.
    #[test]
    fn test_markers__setting_position_raw() {
        let template = TemplatedFile::from_string("foobar".to_string());
        // Check inference in the template
        assert_eq!(template.get_line_pos_of_char_pos(2, true), (1, 3));
        assert_eq!(template.get_line_pos_of_char_pos(2, false), (1, 3));
        // Now check it passes through
        let pos = PositionMarker::new(2..5, 2..5, template, None, None);
        // Can we infer positions correctly
        assert_eq!(pos.working_loc(), (1, 3));
    }

    /// Test that we can correctly set positions manually.
    #[test]
    fn test_markers__setting_position_working() {
        let templ = TemplatedFile::from_string("foobar".to_string());
        let pos = PositionMarker::new(2..5, 2..5, templ, Some(4), Some(4));
        // Can we NOT infer when we're told.
        assert_eq!(pos.working_loc(), (4, 4))
    }

    /// Test that we can correctly compare markers.
    #[test]
    fn test_markers__comparison() {
        let templ = TemplatedFile::from_string("abc".to_string());

        // Assuming start and end are usize, based on typical Rust slicing/indexing.
        let a_pos = PositionMarker::new(0..1, 0..1, templ.clone(), None, None);
        let b_pos = PositionMarker::new(1..2, 1..2, templ.clone(), None, None);
        let c_pos = PositionMarker::new(2..3, 2..3, templ.clone(), None, None);

        let all_pos = vec![&a_pos, &b_pos, &c_pos];

        // Check equality
        assert!(all_pos.iter().all(|p| p == p));

        // Check inequality
        assert!(a_pos != b_pos && a_pos != c_pos && b_pos != c_pos);

        // TODO Finish these tests
        // Check less than
        assert!(a_pos < b_pos && b_pos < c_pos);
        assert!(!(c_pos < a_pos));

        // Check greater than
        assert!(c_pos > a_pos && c_pos > b_pos);
        assert!(!(a_pos > c_pos));

        // Check less than or equal
        assert!(all_pos.iter().all(|p| a_pos <= **p));

        // Check greater than or equal
        assert!(all_pos.iter().all(|p| c_pos >= **p));
    }
}
