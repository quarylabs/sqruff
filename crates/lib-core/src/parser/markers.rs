use std::ops::Range;
use std::rc::Rc;

use ahash::AHashSet;

use crate::slice_helpers::zero_slice;
use crate::templaters::base::TemplatedFile;

/// A reference to a position in a file.
///
/// Things to note:
/// - This combines the previous functionality of FilePositionMarker and
///   EnrichedFilePositionMarker. Additionally it contains a reference to the
///   original templated file.
/// - It no longer explicitly stores a line number or line position in the
///   source or template. This is extrapolated from the templated file as
///   required.
/// - Positions in the source and template are with slices and therefore
///   identify ranges.
/// - Positions within the fixed file are identified with a line number and line
///   position, which identify a point.
/// - Arithmetic comparisons are on the location in the fixed file.
#[derive(Debug, Clone)]
pub struct PositionMarker {
    data: Rc<PositionMarkerData>,
}

impl std::ops::Deref for PositionMarker {
    type Target = PositionMarkerData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl std::ops::DerefMut for PositionMarker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Rc::make_mut(&mut self.data)
    }
}

impl Eq for PositionMarker {}

#[derive(Debug, Clone)]
pub struct PositionMarkerData {
    pub source_slice: Range<usize>,
    pub templated_slice: Range<usize>,
    pub templated_file: TemplatedFile,
    pub working_line_no: usize,
    pub working_line_pos: usize,
}

impl Default for PositionMarker {
    fn default() -> Self {
        Self {
            data: PositionMarkerData {
                source_slice: 0..0,
                templated_slice: 0..0,
                templated_file: "".to_string().into(),
                working_line_no: 0,
                working_line_pos: 0,
            }
            .into(),
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
        match (working_line_no, working_line_pos) {
            (Some(working_line_no), Some(working_line_pos)) => Self {
                data: PositionMarkerData {
                    source_slice,
                    templated_slice,
                    templated_file,
                    working_line_no,
                    working_line_pos,
                }
                .into(),
            },
            _ => {
                let (working_line_no, working_line_pos) =
                    templated_file.get_line_pos_of_char_pos(templated_slice.start, false);
                Self {
                    data: PositionMarkerData {
                        source_slice,
                        templated_slice,
                        templated_file,
                        working_line_no,
                        working_line_pos,
                    }
                    .into(),
                }
            }
        }
    }

    #[track_caller]
    pub fn source_str(&self) -> &str {
        &self.templated_file.source_str[self.source_slice.clone()]
    }

    pub fn line_no(&self) -> usize {
        self.source_position().0
    }

    pub fn line_pos(&self) -> usize {
        self.source_position().1
    }

    #[track_caller]
    pub fn from_child_markers<'a>(
        markers: impl Iterator<Item = &'a PositionMarker>,
    ) -> PositionMarker {
        let mut source_start = usize::MAX;
        let mut source_end = usize::MIN;
        let mut template_start = usize::MAX;
        let mut template_end = usize::MIN;
        let mut templated_files = AHashSet::new();

        for marker in markers {
            source_start = source_start.min(marker.source_slice.start);
            source_end = source_end.max(marker.source_slice.end);
            template_start = template_start.min(marker.templated_slice.start);
            template_end = template_end.max(marker.templated_slice.end);
            templated_files.insert(marker.templated_file.clone());
        }

        if templated_files.len() != 1 {
            panic!("Attempted to make a parent marker from multiple files.");
        }

        let templated_file = templated_files.into_iter().next().unwrap();
        PositionMarker::new(
            source_start..source_end,
            template_start..template_end,
            templated_file,
            None,
            None,
        )
    }

    /// Return the line and position of this marker in the source.
    pub fn source_position(&self) -> (usize, usize) {
        self.templated_file
            .get_line_pos_of_char_pos(self.templated_slice.start, true)
    }

    /// Return the line and position of this marker in the source.
    pub fn templated_position(&self) -> (usize, usize) {
        self.templated_file
            .get_line_pos_of_char_pos(self.templated_slice.start, false)
    }

    pub fn working_loc_after(&self, raw: &str) -> (usize, usize) {
        Self::infer_next_position(raw, self.working_line_no, self.working_line_pos)
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
        Self::new(
            zero_slice(source_point),
            zero_slice(templated_point),
            templated_file,
            working_line_no,
            working_line_pos,
        )
    }

    /// Get a point marker from the start.
    pub fn start_point_marker(&self) -> PositionMarker {
        PositionMarker::from_point(
            self.source_slice.start,
            self.templated_slice.start,
            self.templated_file.clone(),
            // Start points also pass on the working position
            Some(self.working_line_no),
            Some(self.working_line_pos),
        )
    }

    pub fn end_point_marker(&self) -> PositionMarker {
        // Assuming PositionMarker is a struct and from_point is an associated function
        PositionMarker::from_point(
            self.source_slice.end,
            self.templated_slice.end,
            self.templated_file.clone(),
            None,
            None,
        )
    }

    /// Infer literalness from context.
    ///
    /// is_literal should return True if a fix can be applied across
    /// this area in the templated file while being confident that
    /// the fix is still appropriate in the source file. This
    /// obviously applies to any slices which are the same in the
    /// source and the templated files. Slices which are zero-length
    /// in the source are also SyntaxKind::Literal because they can't be
    /// "broken" by any fixes, because they don't exist in the source.
    /// This includes meta segments and any segments added during
    /// the fixing process.
    ///
    /// This value is used for:
    ///     - Ignoring linting errors in templated sections.
    ///     - Whether `iter_patches` can return without recursing.
    ///     - Whether certain rules (such as JJ01) are triggered.
    pub fn is_literal(&self) -> bool {
        self.templated_file
            .is_source_slice_literal(&self.source_slice)
    }

    pub fn from_points(
        start_point_marker: &PositionMarker,
        end_point_marker: &PositionMarker,
    ) -> PositionMarker {
        Self {
            data: PositionMarkerData {
                source_slice: start_point_marker.source_slice.start
                    ..end_point_marker.source_slice.end,
                templated_slice: start_point_marker.templated_slice.start
                    ..end_point_marker.templated_slice.end,
                templated_file: start_point_marker.templated_file.clone(),
                working_line_no: start_point_marker.working_line_no,
                working_line_pos: start_point_marker.working_line_pos,
            }
            .into(),
        }
    }

    pub(crate) fn with_working_position(
        mut self,
        line_no: usize,
        line_pos: usize,
    ) -> PositionMarker {
        self.working_line_no = line_no;
        self.working_line_pos = line_pos;
        self
    }

    pub(crate) fn is_point(&self) -> bool {
        self.source_slice.is_empty() && self.templated_slice.is_empty()
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
    use std::ops::Range;

    use crate::parser::markers::PositionMarker;
    use crate::templaters::base::TemplatedFile;

    /// Test that we can correctly infer positions from strings.
    #[test]
    fn test_markers_infer_next_position() {
        struct Test {
            raw: String,
            start: Range<usize>,
            end: Range<usize>,
        }

        let tests: Vec<Test> = vec![
            Test {
                raw: "fsaljk".to_string(),
                start: 0..0,
                end: 0..6,
            },
            Test {
                raw: "".to_string(),
                start: 2..2,
                end: 2..2,
            },
            Test {
                raw: "\n".to_string(),
                start: 2..2,
                end: 3..1,
            },
            Test {
                raw: "boo\n".to_string(),
                start: 2..2,
                end: 3..1,
            },
            Test {
                raw: "boo\nfoo".to_string(),
                start: 2..2,
                end: 3..4,
            },
            Test {
                raw: "\nfoo".to_string(),
                start: 2..2,
                end: 3..4,
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
    fn test_markers_setting_position_raw() {
        let template: TemplatedFile = "foobar".into();
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
    fn test_markers_setting_position_working() {
        let templ: TemplatedFile = "foobar".into();
        let pos = PositionMarker::new(2..5, 2..5, templ, Some(4), Some(4));
        // Can we NOT infer when we're told.
        assert_eq!(pos.working_loc(), (4, 4))
    }

    /// Test that we can correctly compare markers.
    #[test]
    fn test_markers_comparison() {
        let templ: TemplatedFile = "abc".into();

        // Assuming start and end are usize, based on typical Rust slicing/indexing.
        let a_pos = PositionMarker::new(0..1, 0..1, templ.clone(), None, None);
        let b_pos = PositionMarker::new(1..2, 1..2, templ.clone(), None, None);
        let c_pos = PositionMarker::new(2..3, 2..3, templ.clone(), None, None);

        let all_pos = [&a_pos, &b_pos, &c_pos];

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
