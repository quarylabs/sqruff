use crate::core::linter::linted_file::LintedFile;

#[derive(Debug)]
pub struct LintedDir {
    pub files: Vec<LintedFile>,
    pub path: String,
}

impl LintedDir {
    pub fn new(path: String) -> Self {
        LintedDir { files: vec![], path }
    }

    pub fn add(&mut self, file: LintedFile) {
        self.files.push(file);
    }
}
