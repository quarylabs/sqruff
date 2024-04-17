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
pub struct AnchorEditInfo {
    pub delete: usize,
    pub replace: usize,
    pub create_before: usize,
    pub create_after: usize,
    pub fixes: Vec<LintFix>,
    pub source_fixes: Vec<LintFix>,
    // First fix of edit_type "replace" in "fixes"
    pub first_replace_fix: Option<LintFix>,
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
            first_replace_fix: None,
        }
    }
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
            if let Some(_edit) = fix.edit {
                todo!()
                //     // is_just_source_edit confirms there will be a list
                //     // so we can hint that to mypy.
                //     self.source_fixes.push(edit[0].source_fixes.clone());
                //
                //     // is there already a replace?
                //     if self.first_replace_fix.is_some() {
                //         assert!(self.first_replace_fix.unwrap().edit.
                // is_some());         // is_just_source_edit
                // confirms there will be a list         // and
                // that's the only way to get into _first_replace
                //         // if it's populated so we can hint that to mypy.
                //         // TODO Implement this
                //         //                 linter_logger.info(
                //         //                     "Multiple edits detected,
                // condensing %s onto %s",         //
                // fix,         //
                // self._first_replace,         //
                // )         self._first_replace.edit[0] =
                // self._first_replace.edit[0].edit(&self.source_fixes.clone());
                //         // TODO
                //         //                 linter_logger.info("Condensed fix:
                // %s", self._first_replace)         return;
                //     }
                // } else {
                //     panic!("Fix has no edit: {:?}", fix)
                // }
            }
        }

        self.fixes.push(fix.clone());
        if fix.edit_type == EditType::Replace && self.first_replace_fix.is_none() {
            self.first_replace_fix = Some(fix.clone());
        }
        match fix.edit_type {
            EditType::CreateBefore => self.create_before += 1,
            EditType::CreateAfter => self.create_after += 1,
            EditType::Replace => self.replace += 1,
            EditType::Delete => self.delete += 1,
        };
    }
}
