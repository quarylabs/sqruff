use crate::core::rules::base::LintFix;
use std::ops::Range;

/// A stored reference to a fix in the non-templated file.
#[derive(Debug, Clone)]
pub struct SourceFix {
    edit: String,
    source_slice: Range<usize>,
    // TODO: It might be possible to refactor this to not require
    // a templated_slice (because in theory it's unnecessary).
    // However much of the fix handling code assumes we need
    // a position in the templated file to interpret it.
    // More work required to achieve that if desired.
    templated_slice: Range<usize>,
}

impl SourceFix {
    pub fn new(edit: String, source_slice: Range<usize>, templated_slice: Range<usize>) -> Self {
        SourceFix {
            edit,
            source_slice,
            templated_slice,
        }
    }
}

/// An edit patch for a source file.
#[derive(Clone, Debug)]
pub struct FixPatch {
    templated_slice: Range<usize>,
    fixed_raw: String,
    // The patch category, functions mostly for debugging and explanation
    // than for function. It allows traceability of *why* this patch was
    // generated. It has no significance for processing.
    patch_category: String,
    source_slice: Range<usize>,
    templated_str: String,
    source_str: String,
}

impl FixPatch {
    pub fn new(
        templated_slice: Range<usize>,
        fixed_raw: String,
        patch_category: String,
        source_slice: Range<usize>,
        templated_str: String,
        source_str: String,
    ) -> Self {
        FixPatch {
            templated_slice,
            fixed_raw,
            patch_category,
            source_slice,
            templated_str,
            source_str,
        }
    }

    /// Generate a tuple of this fix for deduping.
    pub fn dedupe_tuple(&self) -> (Range<usize>, String) {
        (self.source_slice.clone(), self.fixed_raw.clone())
    }
}

/// For a given fix anchor, count of the fix edit types and fixes for it."""
pub struct AnchorEditInfo {
    delete: usize,
    replace: usize,
    create_before: usize,
    create_after: usize,
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
    /// Returns total count of fixes.
    fn total(&self) -> usize {
        self.delete + self.replace + self.create_before + self.create_after
    }

    /// Returns True if valid combination of fixes for anchor.
    ///
    /// Cases:
    /// * 0-1 fixes of any type: Valid
    /// * 2 fixes: Valid if and only if types are create_before and create_after
    fn is_valid(&self) -> bool {
        let total = self.total();
        return if total <= 1 {
            // Definitely valid (i.e. no conflict) if 0 or 1. In practice, this
            // function probably won't be called if there are 0 fixes, but 0 is
            // valid; it simply means "no fixes to apply".
            true
        } else if total == 2 {
            // This is only OK for this special case. We allow this because
            // the intent is clear (i.e. no conflict): Insert something *before*
            // the segment and something else *after* the segment.
            self.create_before == 1 && self.create_after == 1
        } else {
            // Definitely bad if > 2.
            false
        };
    }
}
