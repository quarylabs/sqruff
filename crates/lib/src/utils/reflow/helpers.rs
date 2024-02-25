use itertools::Itertools;

use crate::core::rules::base::{LintFix, LintResult};

/// Return a list of fixes from an iterable of LintResult.
pub fn fixes_from_results(results: impl Iterator<Item = LintResult>) -> Vec<LintFix> {
    results.into_iter().flat_map(|result| result.fixes).collect_vec()
}
