use ignore::gitignore::Gitignore;
use std::path::Path;

/// The name of the ignore file that sqruff will look for in the root of the project and use to
/// determine which files to ignore.
const IGNORE_FILE_NAME: &str = ".sqruffignore";

pub(crate) struct IgnoreFile {
    ignore: Gitignore,
}

impl IgnoreFile {
    /// Create a new instance of `IgnoreFile` from the root of the project.
    pub(crate) fn new_from_root(root: &Path) -> Result<Self, String> {
        let ignore_file = root.join(IGNORE_FILE_NAME);
        if ignore_file.exists() {
            let ignore = Gitignore::new(ignore_file);
            match ignore {
                (ignore, None) => Ok(IgnoreFile { ignore }),
                (_, Some(err)) => Err(err.to_string()),
            }
        } else {
            Ok(IgnoreFile {
                ignore: Gitignore::empty(),
            })
        }
    }

    /// Check if the given path should be ignored.
    pub(crate) fn is_ignored(&self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        self.ignore.matched(path, is_dir).is_ignore()
    }
}
