#[derive(Debug, Clone)]
pub struct LintResult {
    pub fix: Vec<LintFix>,
}

impl Default for LintResult {
    fn default() -> Self {
        Self { fix: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct LintFix {}
