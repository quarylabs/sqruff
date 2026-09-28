use super::linted_file::LintedFile;

#[derive(Debug)]
pub struct LintingResult {
    files: Vec<LintedFile>,
    files_skipped: usize,
}

impl LintingResult {
    pub fn new(files: Vec<LintedFile>) -> Self {
        Self::new_with_files_skipped(files, 0)
    }

    pub fn new_with_files_skipped(files: Vec<LintedFile>, files_skipped: usize) -> Self {
        LintingResult {
            files,
            files_skipped,
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn files_skipped(&self) -> usize {
        self.files_skipped
    }

    pub fn has_unfixable_violations(&self) -> bool {
        self.files
            .iter()
            .any(|file| file.has_unfixable_violations())
    }

    pub fn has_violations(&self) -> bool {
        self.files.iter().any(|file| file.has_violations())
    }

    pub fn has_fixable_violations(&self) -> bool {
        self.files.iter().any(|file| file.has_fixable_violations())
    }
}

impl IntoIterator for LintingResult {
    type Item = LintedFile;
    type IntoIter = std::vec::IntoIter<LintedFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}
