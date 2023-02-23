pub struct LintResult {
    pub fix: Vec<LintFix>,
}

impl Default for LintResult {
    fn default() -> Self {
        Self { fix: Vec::new() }
    }
}

pub struct LintFix {}
