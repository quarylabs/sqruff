use crate::core::config::FluffConfig;
use crate::core::linter::linting_result::LintingResult;

pub struct Linter {
    config: FluffConfig,
}

impl Linter {
    pub fn new(config: FluffConfig) -> Linter {
        Linter { config }
    }

    /// Lint strings directly.
    pub fn lint_string_wrapped(
        &self,
        sql: String,
        f_name: Option<String>,
        fix: Option<bool>,
    ) -> LintingResult {
        // TODO Translate LintingResult
        panic!("Not implemented yet.")
    }
}
