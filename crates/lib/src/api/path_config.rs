use std::path::PathBuf;

use super::workspace::PathDiscoveryOptions;
use crate::config::FluffConfig;

impl<'a> PathDiscoveryOptions<'a> {
    pub fn from_config(working_dir: PathBuf, config: &'a FluffConfig) -> Self {
        Self {
            ignore_file_name: ".sqruffignore",
            ignore_non_existent_files: false,
            ignore_files: true,
            working_dir,
            file_exts: config.sql_file_exts(),
            ignorer: None,
        }
    }
}
