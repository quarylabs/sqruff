use ignore::gitignore::Gitignore;
use std::path::Path;

/// The name of the ignore file that sqruff will look for in the root of the project and use to
/// determine which files to ignore.
const IGNORE_FILE_NAME: &str = ".sqruffignore";

pub struct IgnoreFile {
    ignore: Gitignore,
}

impl IgnoreFile {
    /// Create a new instance of `IgnoreFile` from the root of the project.
    pub fn new_from_root(root: &Path) -> Result<Self, String> {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let ignore_file = root.join(IGNORE_FILE_NAME);
        if ignore_file.exists() {
            let ignore = Gitignore::new(ignore_file);
            match ignore {
                (ignore, None) => Ok(IgnoreFile { ignore }),
                (_, Some(err)) => Err(err.to_string()),
            }
        } else {
            Ok(Self::empty())
        }
    }

    pub fn empty() -> Self {
        Self {
            ignore: Gitignore::empty(),
        }
    }

    /// Check if the given path should be ignored.
    pub fn is_ignored(&self, path: &Path) -> bool {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let is_dir = path.is_dir();
        let match_result = if path.starts_with(self.ignore.path()) {
            self.ignore.matched_path_or_any_parents(&path, is_dir)
        } else {
            self.ignore.matched(&path, is_dir)
        };
        let is_ignored = match_result.is_ignore();

        if is_ignored {
            let path_type = if is_dir { "directory" } else { "file" };
            log::debug!(
                "Ignoring {} '{}' due to ignore pattern",
                path_type,
                path.display()
            );

            if let Some(pattern) = match_result.inner() {
                log::debug!("Matched ignore pattern: '{}'", pattern.original());
            }
        }

        is_ignored
    }
}
