use super::linted_file::LintedFile;

#[derive(Debug)]
pub struct LintingResult {
    files: Vec<LintedFile>,
}

impl LintingResult {
    pub fn new(files: Vec<LintedFile>) -> Self {
        LintingResult { files }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn has_unfixable_violations(&self) -> bool {
        self.files
            .iter()
            .any(|file| file.has_unfixable_violations())
    }

    pub fn has_violations(&self) -> bool {
        self.files.iter().any(|file| file.has_violations())
    }
}

impl IntoIterator for LintingResult {
    type Item = LintedFile;
    type IntoIter = std::vec::IntoIter<LintedFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}
