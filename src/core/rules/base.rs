use crate::core::parser::segments::base::Segment;

#[derive(Debug, Clone)]
pub struct LintResult {
    pub fix: Vec<LintFix>,
}

impl Default for LintResult {
    fn default() -> Self {
        Self { fix: Vec::new() }
    }
}

/// One of `create_before`, `create_after`, `replace`, `delete` to indicate the kind of fix required.
#[derive(Debug, Clone, PartialEq)]
pub enum EditType {
    CreateBefore,
    CreateAfter,
    Replace,
    Delete,
}

/// A class to hold a potential fix to a linting violation.
///
///     Args:
///         edit_type (:obj:`str`): One of `create_before`, `create_after`,
///             `replace`, `delete` to indicate the kind of fix this represents.
///         anchor (:obj:`BaseSegment`): A segment which represents
///             the *position* that this fix should be applied at. For deletions
///             it represents the segment to delete, for creations it implies the
///             position to create at (with the existing element at this position
///             to be moved *after* the edit), for a `replace` it implies the
///             segment to be replaced.
///         edit (iterable of :obj:`BaseSegment`, optional): For `replace` and
///             `create` fixes, this holds the iterable of segments to create
///             or replace at the given `anchor` point.
///         source (iterable of :obj:`BaseSegment`, optional): For `replace` and
///             `create` fixes, this holds iterable of segments that provided
///             code. IMPORTANT: The linter uses this to prevent copying material
///             from templated areas.
#[derive(Debug, Clone)]
pub struct LintFix {
    pub edit_type: EditType,
    pub anchor: Box<dyn Segment>,
    pub edit: Option<Vec<Box<dyn Segment>>>,
    source: Vec<Box<dyn Segment>>,
}

impl LintFix {
    fn new(
        edit_type: EditType,
        anchor: Box<dyn Segment>,
        edit: Option<Vec<Box<dyn Segment>>>,
        source: Option<Vec<Box<dyn Segment>>>,
    ) -> Self {
        // If `edit` is provided, copy all elements and strip position markers.
        let mut clean_edit = None;
        if let Some(mut edit) = edit {
            // Developer Note: Ensure position markers are unset for all edit segments.
            // We rely on realignment to make position markers later in the process.
            for seg in &mut edit {
                if seg.get_position_marker().is_some() {
                    // assuming `pos_marker` is a field of `BaseSegment`
                    eprintln!("Developer Note: Edit segment found with preset position marker. These should be unset and calculated later.");
                    // assuming `pos_marker` is Option-like and can be set to None
                    seg.set_position_marker(None);
                };
            }
            clean_edit = Some(edit);
        }

        // If `source` is provided, filter segments with position markers.
        let clean_source = source.map_or(Vec::new(), |source| {
            source
                .into_iter()
                .filter(|seg| seg.get_position_marker().is_some())
                .collect()
        });

        LintFix {
            edit_type,
            anchor,
            edit: clean_edit,
            source: clean_source,
        }
    }

    pub fn replace(
        anchor_segment: Box<dyn Segment>,
        edit_segments: Vec<Box<dyn Segment>>,
        source: Option<Vec<Box<dyn Segment>>>,
    ) -> Self {
        Self::new(
            EditType::Replace,
            anchor_segment,
            Some(edit_segments),
            source,
        )
    }

    /// Return whether this a valid source only edit.
    pub fn is_just_source_edit(&self) -> bool {
        if let Some(edit) = &self.edit {
            self.edit_type == EditType::Replace
                && edit.len() == 1
                && edit[0].get_raw() == self.anchor.get_raw()
        } else {
            false
        }
    }
}

impl PartialEq for LintFix {
    fn eq(&self, other: &Self) -> bool {
        // Check if edit_types are equal
        if self.edit_type != other.edit_type {
            return false;
        }
        // Check if anchor.class_types are equal
        if self.anchor.get_type() != other.anchor.get_type() {
            return false;
        }
        // Check if anchor.uuids are equal
        if self.anchor.get_uuid() != other.anchor.get_uuid() {
            return false;
        }
        // Compare edits if they exist
        if let Some(self_edit) = &self.edit {
            if let Some(other_edit) = &other.edit {
                // Check lengths
                if self_edit.len() != other_edit.len() {
                    return false;
                }
                // Compare raw and source_fixes for each corresponding BaseSegment
                for (self_base_segment, other_base_segment) in self_edit.iter().zip(other_edit) {
                    if self_base_segment.get_raw() != other_base_segment.get_raw()
                        || self_base_segment.get_source_fixes()
                            != other_base_segment.get_source_fixes()
                    {
                        return false;
                    }
                }
            } else {
                // self has edit, other doesn't
                return false;
            }
        } else if other.edit.is_some() {
            // other has edit, self doesn't
            return false;
        }
        // If none of the above conditions were met, objects are equal
        true
    }
}
