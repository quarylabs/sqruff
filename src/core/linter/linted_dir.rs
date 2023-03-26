// use sqlfluff::linted_file::LintedFile;

pub struct LintedDir {
    // pub files: Vec<LintedFile>,
    pub path: String,
}

impl LintedDir {
    pub fn new(path: &str) -> Self {
        LintedDir {
            // files: vec![],
            path: path.to_string(),
        }
    }
}
