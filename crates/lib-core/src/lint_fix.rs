use std::ops::Range;

use ahash::AHashSet;

use crate::parser::segments::ErasedSegment;
use crate::templaters::{RawFileSlice, TemplatedFile};

/// A potential fix to a linting violation.
#[derive(Debug, Clone)]
pub enum LintFix {
    /// Create a fix before an anchor segment
    CreateBefore {
        /// The segment before which to create the edit
        anchor: ErasedSegment,
        /// The segments to create
        edit: Vec<ErasedSegment>,
    },
    /// Create a fix after an anchor segment
    CreateAfter {
        /// The segment after which to create the edit
        anchor: ErasedSegment,
        /// The segments to create
        edit: Vec<ErasedSegment>,
        /// Segments that provided the source code
        source: Vec<ErasedSegment>,
    },
    /// Replace an anchor segment with new segments
    Replace {
        /// The segment to replace
        anchor: ErasedSegment,
        /// The segments to replace with
        edit: Vec<ErasedSegment>,
        /// Segments that provided the source code
        source: Vec<ErasedSegment>,
    },
    /// Delete an anchor segment
    Delete {
        /// The segment to delete
        anchor: ErasedSegment,
    },
}

impl LintFix {
    /// Get the anchor segment for this fix
    pub fn anchor(&self) -> &ErasedSegment {
        match self {
            LintFix::CreateBefore { anchor, .. } => anchor,
            LintFix::CreateAfter { anchor, .. } => anchor,
            LintFix::Replace { anchor, .. } => anchor,
            LintFix::Delete { anchor } => anchor,
        }
    }

    fn prepare_edit_segments(mut edit: Vec<ErasedSegment>) -> Vec<ErasedSegment> {
        // Copy all elements and strip position markers.
        // Developer Note: Ensure position markers are unset for all edit segments.
        // We rely on realignment to make position markers later in the process.
        for seg in &mut edit {
            if seg.get_position_marker().is_some() {
                seg.make_mut().set_position_marker(None);
            };
        }
        edit
    }

    fn prepare_source_segments(source: Option<Vec<ErasedSegment>>) -> Vec<ErasedSegment> {
        // If `source` is provided, filter segments with position markers.
        source.map_or(Vec::new(), |source| {
            source
                .into_iter()
                .filter(|seg| seg.get_position_marker().is_some())
                .collect()
        })
    }

    pub fn create_before(anchor: ErasedSegment, edit_segments: Vec<ErasedSegment>) -> Self {
        LintFix::CreateBefore {
            anchor,
            edit: Self::prepare_edit_segments(edit_segments),
        }
    }

    pub fn create_after(
        anchor: ErasedSegment,
        edit_segments: Vec<ErasedSegment>,
        source: Option<Vec<ErasedSegment>>,
    ) -> Self {
        LintFix::CreateAfter {
            anchor,
            edit: Self::prepare_edit_segments(edit_segments),
            source: Self::prepare_source_segments(source),
        }
    }

    pub fn replace(
        anchor_segment: ErasedSegment,
        edit_segments: Vec<ErasedSegment>,
        source: Option<Vec<ErasedSegment>>,
    ) -> Self {
        LintFix::Replace {
            anchor: anchor_segment,
            edit: Self::prepare_edit_segments(edit_segments),
            source: Self::prepare_source_segments(source),
        }
    }

    pub fn delete(anchor_segment: ErasedSegment) -> Self {
        LintFix::Delete {
            anchor: anchor_segment,
        }
    }

    /// Return whether this a valid source only edit.
    pub fn is_just_source_edit(&self) -> bool {
        match self {
            LintFix::Replace { anchor, edit, .. } => {
                edit.len() == 1 && edit[0].raw() == anchor.raw()
            }
            _ => false,
        }
    }

    fn fix_slices(
        &self,
        templated_file: &TemplatedFile,
        within_only: bool,
    ) -> AHashSet<RawFileSlice> {
        let anchor = match self {
            LintFix::CreateBefore { anchor, .. } => anchor,
            LintFix::CreateAfter { anchor, .. } => anchor,
            LintFix::Replace { anchor, .. } => anchor,
            LintFix::Delete { anchor } => anchor,
        };

        let anchor_slice = anchor
            .get_position_marker()
            .unwrap()
            .templated_slice
            .clone();

        let adjust_boundary = if !within_only { 1 } else { 0 };

        let templated_slice = match self {
            LintFix::CreateBefore { .. } => {
                anchor_slice.start.saturating_sub(1)..anchor_slice.start + adjust_boundary
            }
            LintFix::CreateAfter { .. } => anchor_slice.end - adjust_boundary..anchor_slice.end + 1,
            LintFix::Replace { anchor, edit, .. } => {
                let pos = anchor.get_position_marker().unwrap();
                if pos.source_slice.start == pos.source_slice.end {
                    return AHashSet::new();
                } else if edit
                    .iter()
                    .all(|it| it.segments().is_empty() && !it.get_source_fixes().is_empty())
                {
                    let source_edit_slices: Vec<_> = edit
                        .iter()
                        .flat_map(|edit| edit.get_source_fixes())
                        .map(|source_fixe| source_fixe.source_slice.clone())
                        .collect();

                    let slice =
                        templated_file.raw_slices_spanning_source_slice(&source_edit_slices[0]);
                    return AHashSet::from_iter(slice);
                }

                anchor_slice
            }
            LintFix::Delete { .. } => anchor_slice,
        };

        self.raw_slices_from_templated_slices(
            templated_file,
            std::iter::once(templated_slice),
            RawFileSlice::new(String::new(), "literal".to_string(), usize::MAX, None, None).into(),
        )
    }

    fn raw_slices_from_templated_slices(
        &self,
        templated_file: &TemplatedFile,
        templated_slices: impl Iterator<Item = Range<usize>>,
        file_end_slice: Option<RawFileSlice>,
    ) -> AHashSet<RawFileSlice> {
        let mut raw_slices = AHashSet::new();

        for templated_slice in templated_slices {
            let templated_slice =
                templated_file.templated_slice_to_source_slice(templated_slice.clone());

            match templated_slice {
                Ok(templated_slice) => raw_slices
                    .extend(templated_file.raw_slices_spanning_source_slice(&templated_slice)),
                Err(_) => {
                    if let Some(file_end_slice) = file_end_slice.clone() {
                        raw_slices.insert(file_end_slice);
                    }
                }
            }
        }

        raw_slices
    }

    pub fn has_template_conflicts(&self, templated_file: &TemplatedFile) -> bool {
        if let LintFix::Replace { anchor, edit, .. } = self
            && edit.len() == 1
        {
            let edit_seg = &edit[0];
            if edit_seg.raw() == anchor.raw() && !edit_seg.get_source_fixes().is_empty() {
                return false;
            }
        }

        let check_fn = match self {
            LintFix::CreateAfter { .. } | LintFix::CreateBefore { .. } => itertools::all,
            _ => itertools::any,
        };

        let fix_slices = self.fix_slices(templated_file, false);
        let result = check_fn(fix_slices, |fs: RawFileSlice| fs.slice_type == "templated");

        let source_is_empty = match self {
            LintFix::CreateAfter { source, .. } | LintFix::Replace { source, .. } => {
                source.is_empty()
            }
            _ => true,
        };

        if result || source_is_empty {
            return result;
        }

        let templated_slices = None;
        let raw_slices = self.raw_slices_from_templated_slices(
            templated_file,
            templated_slices.into_iter(),
            None,
        );
        raw_slices.iter().any(|fs| fs.slice_type == "templated")
    }
}

impl PartialEq for LintFix {
    fn eq(&self, other: &Self) -> bool {
        use std::mem::discriminant;

        // Check if variants are the same
        if discriminant(self) != discriminant(other) {
            return false;
        }

        match (self, other) {
            (
                LintFix::CreateBefore {
                    anchor: a1,
                    edit: e1,
                },
                LintFix::CreateBefore {
                    anchor: a2,
                    edit: e2,
                },
            ) => {
                a1.get_type() == a2.get_type()
                    && a1.id() == a2.id()
                    && e1.len() == e2.len()
                    && e1.iter().zip(e2).all(|(s1, s2)| {
                        s1.raw() == s2.raw() && s1.get_source_fixes() == s2.get_source_fixes()
                    })
            }
            (
                LintFix::CreateAfter {
                    anchor: a1,
                    edit: e1,
                    source: s1,
                },
                LintFix::CreateAfter {
                    anchor: a2,
                    edit: e2,
                    source: s2,
                },
            ) => {
                a1.get_type() == a2.get_type()
                    && a1.id() == a2.id()
                    && e1.len() == e2.len()
                    && e1.iter().zip(e2).all(|(seg1, seg2)| {
                        seg1.raw() == seg2.raw()
                            && seg1.get_source_fixes() == seg2.get_source_fixes()
                    })
                    && s1 == s2
            }
            (
                LintFix::Replace {
                    anchor: a1,
                    edit: e1,
                    source: s1,
                },
                LintFix::Replace {
                    anchor: a2,
                    edit: e2,
                    source: s2,
                },
            ) => {
                a1.get_type() == a2.get_type()
                    && a1.id() == a2.id()
                    && e1.len() == e2.len()
                    && e1.iter().zip(e2).all(|(seg1, seg2)| {
                        seg1.raw() == seg2.raw()
                            && seg1.get_source_fixes() == seg2.get_source_fixes()
                    })
                    && s1 == s2
            }
            (LintFix::Delete { anchor: a1 }, LintFix::Delete { anchor: a2 }) => {
                a1.get_type() == a2.get_type() && a1.id() == a2.id()
            }
            _ => false,
        }
    }
}
