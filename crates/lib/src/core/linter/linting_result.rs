use super::linted_file::LintedFile;

#[derive(Debug)]
pub struct LintingResult {
    pub files: Vec<LintedFile>,
}

impl Default for LintingResult {
    fn default() -> Self {
        Self::new()
    }
}

impl LintingResult {
    pub fn new() -> Self {
        LintingResult { files: vec![] }
    }

    pub fn has_violations(&self) -> bool {
        self.files.iter().any(|file| file.has_violations())
    }

    /// Add a new `LintedDir` to this result.
    pub fn add(&mut self, path: LintedFile) -> usize {
        let idx = self.files.len();
        self.files.push(path);
        idx
    }
}
