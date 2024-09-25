use append_only_vec::AppendOnlyVec;

use crate::core::linter::linted_file::LintedFile;

#[derive(Debug)]
pub struct LintedDir {
    pub files: AppendOnlyVec<LintedFile>,
    pub path: String,
}

impl LintedDir {
    pub fn new(path: String) -> Self {
        LintedDir {
            files: AppendOnlyVec::new(),
            path,
        }
    }

    pub fn add(&self, file: LintedFile) {
        self.files.push(file);
    }
}
