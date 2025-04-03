use crate::core::linter::linted_dir::LintedDir;

#[derive(Debug)]
pub struct LintingResult {
    pub paths: Vec<LintedDir>,
}

impl Default for LintingResult {
    fn default() -> Self {
        Self::new()
    }
}

impl LintingResult {
    pub fn new() -> Self {
        LintingResult { paths: vec![] }
    }

    /// Add a new `LintedDir` to this result.
    pub fn add(&mut self, path: LintedDir) -> usize {
        let idx = self.paths.len();
        self.paths.push(path);
        idx
    }
}
