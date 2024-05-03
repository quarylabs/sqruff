use std::ops::Range;

use crate::core::rules::base::{EditType, LintFix};

/// A stored reference to a fix in the non-templated file.
#[derive(Hash, Debug, Clone, PartialEq)]
pub struct SourceFix {
    pub(crate) edit: String,
    pub(crate) source_slice: Range<usize>,
    // TODO: It might be possible to refactor this to not require
    // a templated_slice (because in theory it's unnecessary).
    // However much of the fix handling code assumes we need
    // a position in the templated file to interpret it.
    // More work required to achieve that if desired.
    pub(crate) templated_slice: Range<usize>,
}

impl SourceFix {
    pub fn new(edit: String, source_slice: Range<usize>, templated_slice: Range<usize>) -> Self {
        SourceFix { edit, source_slice, templated_slice }
    }
}

/// An edit patch for a source file.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FixPatch {
    templated_slice: Range<usize>,
    pub fixed_raw: String,
    // The patch category, functions mostly for debugging and explanation
    // than for function. It allows traceability of *why* this patch was
    // generated. It has no significance for processing.
    patch_category: String,
    pub source_slice: Range<usize>,
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
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct AnchorEditInfo {
    pub delete: usize,
    pub replace: usize,
    pub create_before: usize,
    pub create_after: usize,
    pub fixes: Vec<LintFix>,
    pub source_fixes: Vec<SourceFix>,
    // First fix of edit_type "replace" in "fixes"
    pub first_replace: Option<LintFix>,
}

impl AnchorEditInfo {
    /// Returns total count of fixes.
    #[allow(dead_code)]
    fn total(&self) -> usize {
        self.delete + self.replace + self.create_before + self.create_after
    }

    /// Returns True if valid combination of fixes for anchor.
    ///
    /// Cases:
    /// * 0-1 fixes of any type: Valid
    /// * 2 fixes: Valid if and only if types are create_before and create_after
    #[allow(dead_code)]
    fn is_valid(&self) -> bool {
        let total = self.total();
        if total <= 1 {
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
        }
    }

    /// Adds the fix and updates stats.
    ///
    /// We also allow potentially multiple source fixes on the same anchor by
    /// condensing them together here.
    pub fn add(&mut self, fix: LintFix) {
        if self.fixes.contains(&fix) {
            // Deduplicate fixes in case it's already in there.
            return;
        };

        if fix.is_just_source_edit() {
            let edit = fix.edit.as_ref().unwrap();
            self.source_fixes.extend(edit[0].get_source_fixes());

            if let Some(_first_replace) = &self.first_replace {
                unimplemented!();
            }
        }

        self.fixes.push(fix.clone());
        if fix.edit_type == EditType::Replace && self.first_replace.is_none() {
            self.first_replace = Some(fix.clone());
        }
        match fix.edit_type {
            EditType::CreateBefore => self.create_before += 1,
            EditType::CreateAfter => self.create_after += 1,
            EditType::Replace => self.replace += 1,
            EditType::Delete => self.delete += 1,
        };
    }
}
