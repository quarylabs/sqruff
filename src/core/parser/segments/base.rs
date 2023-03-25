use crate::core::rules::base::LintFix;
use std::hash::Hash;

/// A stored reference to a fix in the non-templated file.
#[derive(Debug, Clone, Hash)]
pub struct SourceFix {
    // TODO source_slice and templated_slice are slice types in Python
    edit: String,
    source_slice: String,
    //    TODO: It might be possible to refactor this to not require
    //     a templated_slice (because in theory it's unnecessary).
    //     However much of the fix handling code assumes we need
    //     a position in the templated file to interpret it.
    //     More work required to achieve that if desired.
    templated_slice: String,
}

/// An element of the response to BaseSegment.path_to().
///     Attributes:
///         segment (:obj:`BaseSegment`): The segment in the chain.
///         idx (int): The index of the target within its `segment`.
///         len (int): The number of children `segment` has.
#[derive(Debug, Clone)]
pub struct PathStep {
    segment: BaseSegment,
    idx: usize,
    len: usize,
}

/// An edit patch for a source file.
#[derive(Debug, Clone)]
pub struct FixPatch {
    //. TODO templated_slice and source_slice are of type slices in Python
    templated_slice: String,

    fixed_raw: String,
    /// The patch category, functions mostly for debugging and explanation than for function. It allows traceability of *why* this patch was generated. It has no significance for processing.
    patch_category: String,
    source_slice: String,
    templated_str: String,
    source_str: String,
}

impl FixPatch {
    /// Generate a tuple of this fix for de-duping.
    fn dedupe_tuple(self: &Self) -> (String, String) {
        (self.source_slice.clone(), self.fixed_raw.clone())
    }
}

/// For a given fix anchor, count of the fix edit types and fixes for it."""
pub struct AnchorEditInfo {
    delete: isize,
    replace: isize,
    create_before: isize,
    create_after: isize,
    fixes: Vec<LintFix>,
    source_fixes: Vec<LintFix>,
    // First fix of edit_type "replace" in "fixes"
    _first_replace_fix: Option<LintFix>,
}

impl Default for AnchorEditInfo {
    fn default() -> Self {
        AnchorEditInfo {
            delete: 0,
            replace: 0,
            create_before: 0,
            create_after: 0,
            fixes: Vec::new(),
            source_fixes: Vec::new(),
            _first_replace_fix: None,
        }
    }
}

impl AnchorEditInfo {
    /// Adds the fix and updates stats.
    /// We also allow potentially multiple source fixes on the same
    /// anchor by condensing them together here.
    fn add(self: &mut Self, fix: LintFix) {
        panic!("Not implemented yet");
    }

    /// Returns total count of fixes.
    fn total(self: &Self) -> usize {
        self.fixes.len()
    }

    /// Returns True if valid combination of fixes for anchor.
    ///
    /// Cases:
    /// * 0-1 fixes of any type: Valid
    /// * 2 fixes: Valid if and only if types are create_before and create_after
    fn is_valid(self: &Self) -> bool {
        if self.total() <= 1 {
            // Definitely valid (i.e. no conflict) if 0 or 1. In practice, this
            // function probably won't be called if there are 0 fixes, but 0 is
            // valid; it simply means "no fixes to apply".
            // return true;
        }
        if self.total() == 2 {
            // This is only OK for this special case. We allow this because
            // the intent is clear (i.e. no conflict): Insert something *before*
            // the segment and something else *after* the segment.
            return self.create_before == 1 && self.create_after == 1;
        }
        // Definitely bad if > 2.
        return false;
    }
}

#[derive(Debug, Clone)]
pub struct BaseSegment {}
