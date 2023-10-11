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

//// One of `create_before`, `create_after`, `replace`, `delete` to indicate the kind of fix required.
#[derive(Debug, Clone, PartialEq)]
enum EditType {
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
    edit_type: EditType,
    anchor: Box<dyn Segment>,
    edit: Option<Vec<Box<dyn Segment>>>,
    source: Vec<Box<dyn Segment>>,
}

impl LintFix {
    fn new(
        edit_type: EditType,
        anchor: Box<dyn Segment>,
        edit: Option<Vec<Box<dyn Segment>>>,
        source: Option<Vec<Box<dyn Segment>>>,
    ) -> Self {
        todo!()
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
}

impl PartialEq for LintFix {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}
