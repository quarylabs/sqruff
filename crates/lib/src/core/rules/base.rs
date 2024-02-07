use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::ops::Deref;
use std::rc::Rc;

use super::context::RuleContext;
use super::crawlers::Crawler;
use crate::core::dialects::base::Dialect;
use crate::core::errors::SQLLintError;
use crate::core::parser::segments::base::Segment;

// Assuming BaseSegment, LintFix, and SQLLintError are defined elsewhere.

#[derive(Clone)]
pub struct LintResult {
    anchor: Option<Box<dyn Segment>>,
    pub fixes: Vec<LintFix>,
    memory: Option<HashMap<String, String>>, // Adjust type as needed
    description: Option<String>,
    source: String,
}

impl LintResult {
    pub fn new(
        anchor: Option<Box<dyn Segment>>,
        fixes: Vec<LintFix>,
        memory: Option<HashMap<String, String>>, // Adjust type as needed
        description: Option<String>,
        source: Option<String>,
    ) -> Self {
        // let fixes = fixes.into_iter().filter(|f| !f.is_trivial()).collect();

        LintResult { anchor, fixes, memory, description, source: source.unwrap_or_default() }
    }

    pub fn to_linting_error(&self, rule_description: &'static str) -> Option<SQLLintError> {
        let _anchor = self.anchor.as_ref()?;

        SQLLintError {
            description: self.description.clone().unwrap_or(rule_description.to_string()),
        }
        .into()
    }
}

impl Debug for LintResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.anchor {
            None => write!(f, "LintResult(<empty>)"),
            Some(anchor) => {
                let fix_coda = if !self.fixes.is_empty() {
                    format!("+{}F", self.fixes.len())
                } else {
                    "".to_string()
                };

                match &self.description {
                    Some(desc) => {
                        if !self.source.is_empty() {
                            write!(
                                f,
                                "LintResult({} [{}]: {:?}{})",
                                desc, self.source, anchor, fix_coda
                            )
                        } else {
                            write!(f, "LintResult({}: {:?}{})", desc, anchor, fix_coda)
                        }
                    }
                    None => write!(f, "LintResult({:?}{})", anchor, fix_coda),
                }
            }
        }
    }
}

/// One of `create_before`, `create_after`, `replace`, `delete` to indicate the
/// kind of fix required.
#[derive(Debug, Clone, Copy, PartialEq)]
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
///             it represents the segment to delete, for creations it implies
/// the             position to create at (with the existing element at this
/// position             to be moved *after* the edit), for a `replace` it
/// implies the             segment to be replaced.
///         edit (iterable of :obj:`BaseSegment`, optional): For `replace` and
///             `create` fixes, this holds the iterable of segments to create
///             or replace at the given `anchor` point.
///         source (iterable of :obj:`BaseSegment`, optional): For `replace` and
///             `create` fixes, this holds iterable of segments that provided
///             code. IMPORTANT: The linter uses this to prevent copying
/// material             from templated areas.
#[derive(Debug, Clone)]
pub struct LintFix {
    pub edit_type: EditType,
    pub anchor: Box<dyn Segment>,
    pub edit: Option<Vec<Box<dyn Segment>>>,
    pub source: Vec<Box<dyn Segment>>,
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
                    eprintln!(
                        "Developer Note: Edit segment found with preset position marker. These \
                         should be unset and calculated later."
                    );
                    // assuming `pos_marker` is Option-like and can be set to None
                    seg.set_position_marker(None);
                };
            }
            clean_edit = Some(edit);
        }

        // If `source` is provided, filter segments with position markers.
        let clean_source = source.map_or(Vec::new(), |source| {
            source.into_iter().filter(|seg| seg.get_position_marker().is_some()).collect()
        });

        LintFix { edit_type, anchor, edit: clean_edit, source: clean_source }
    }

    pub fn create_before(anchor: Box<dyn Segment>, edit_segments: Vec<Box<dyn Segment>>) -> Self {
        Self::new(EditType::CreateBefore, anchor, edit_segments.into(), None)
    }

    pub fn create_after(
        anchor: Box<dyn Segment>,
        edit_segments: Vec<Box<dyn Segment>>,
        source: Option<Vec<Box<dyn Segment>>>,
    ) -> Self {
        Self::new(EditType::CreateAfter, anchor, edit_segments.into(), source)
    }

    pub fn replace(
        anchor_segment: Box<dyn Segment>,
        edit_segments: Vec<Box<dyn Segment>>,
        source: Option<Vec<Box<dyn Segment>>>,
    ) -> Self {
        Self::new(EditType::Replace, anchor_segment, Some(edit_segments), source)
    }

    pub fn delete(anchor_segment: Box<dyn Segment>) -> Self {
        Self::new(EditType::Delete, anchor_segment, None, None)
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

pub trait Rule: Debug + 'static {
    fn lint_phase(&self) -> &'static str {
        "main"
    }

    fn description(&self) -> &'static str {
        "write description"
    }

    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult>;

    fn is_fix_compatible(&self) -> bool {
        false
    }

    fn crawl_behaviour(&self) -> Box<dyn Crawler>;

    fn crawl(
        &self,
        dialect: Dialect,
        fix: bool,
        tree: Box<dyn Segment>,
    ) -> (Vec<SQLLintError>, Vec<LintFix>) {
        let root_context = RuleContext { dialect, fix, segment: tree, ..<_>::default() };
        let mut vs = Vec::new();
        let mut fixes = Vec::new();

        for context in self.crawl_behaviour().crawl(root_context) {
            let resp = self.eval(context);

            let mut new_lerrs = Vec::new();
            let mut new_fixes = Vec::new();

            if resp.is_empty() {
                // Assume this means no problems (also means no memory)
            } else {
                for elem in resp {
                    self.process_lint_result(elem, &mut new_lerrs, &mut new_fixes);
                }
            }

            // Consume the new results
            vs.extend(new_lerrs);
            fixes.extend(new_fixes);
        }

        (vs, fixes)
    }

    fn process_lint_result(
        &self,
        res: LintResult,
        new_lerrs: &mut Vec<SQLLintError>,
        new_fixes: &mut Vec<LintFix>,
    ) {
        let ignored = false;

        if let Some(lerr) = res.to_linting_error(self.description()) {
            new_lerrs.push(lerr);
        }

        if !ignored {
            new_fixes.extend(res.fixes);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ErasedRule {
    erased: Rc<dyn Rule>,
}

impl Deref for ErasedRule {
    type Target = dyn Rule;

    fn deref(&self) -> &Self::Target {
        self.erased.as_ref()
    }
}

pub trait Erased {
    type Erased;

    fn erased(self) -> Self::Erased;
}

impl<T: Rule> Erased for T {
    type Erased = ErasedRule;

    fn erased(self) -> Self::Erased {
        ErasedRule { erased: Rc::new(self) }
    }
}
