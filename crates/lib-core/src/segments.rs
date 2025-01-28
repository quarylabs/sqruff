use crate::edit_type::EditType;
use crate::lint_fix::LintFix;
use crate::parser::segments::fix::SourceFix;

type LintFixIdx = usize;

/// For a given fix anchor, count of the fix edit types and fixes for it."""
#[derive(Debug, Default)]
pub struct AnchorEditInfo {
    pub delete: usize,
    pub replace: usize,
    pub create_before: usize,
    pub create_after: usize,
    pub fixes: Vec<LintFix>,
    pub source_fixes: Vec<SourceFix>,
    // First fix of edit_type "replace" in "fixes"
    pub first_replace: Option<LintFixIdx>,
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
            self.source_fixes.extend(fix.edit[0].get_source_fixes());

            if let Some(_first_replace) = &self.first_replace {
                unimplemented!();
            }
        }

        if fix.edit_type == EditType::Replace && self.first_replace.is_none() {
            self.first_replace = Some(self.fixes.len());
        }

        match fix.edit_type {
            EditType::CreateBefore => self.create_before += 1,
            EditType::CreateAfter => self.create_after += 1,
            EditType::Replace => self.replace += 1,
            EditType::Delete => self.delete += 1,
        };

        self.fixes.push(fix);
    }
}
