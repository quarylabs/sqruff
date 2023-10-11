use crate::core::rules::base::{LintFix, LintResult};

/// Return a list of fixes from an iterable of LintResult.
pub fn fixes_from_results(_results: &dyn Iterator<Item = LintResult>) -> Vec<LintFix> {
    panic!("Not implemented");
}
