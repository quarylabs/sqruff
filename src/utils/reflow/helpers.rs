use crate::core::rules::base::{LintFix, LintResult};

/// Return a list of fixes from an iterable of LintResult.
pub fn fixes_from_results(results: &dyn Iterator<Item = LintResult>) -> Vec<LintFix> {
    results.flat_map(|r| r.fixes).collect()
}
