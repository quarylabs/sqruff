//! Discovery methods for SQL files.
//!
//! The main public method here is [`paths_from_path`], which takes a potentially
//! ambiguous path and resolves it into specific file references.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use sqruff_lib_core::helpers;
use walkdir::WalkDir;

/// Return SQL file paths from a potentially ambiguous path.
pub fn paths_from_path(
    path: PathBuf,
    ignore_file_name: Option<String>,
    ignore_non_existent_files: Option<bool>,
    ignore_files: Option<bool>,
    working_path: Option<String>,
    target_file_exts: &[String],
    ignorer: Option<&(dyn Fn(&Path) -> bool + Send + Sync)>,
) -> Vec<String> {
    let ignore_file_name = ignore_file_name.unwrap_or_else(|| String::from(".sqlfluffignore"));
    let ignore_non_existent_files = ignore_non_existent_files.unwrap_or(false);
    let ignore_files = ignore_files.unwrap_or(true);
    let _working_path =
        working_path.unwrap_or_else(|| std::env::current_dir().unwrap().display().to_string());

    let Ok(metadata) = std::fs::metadata(&path) else {
        if ignore_non_existent_files {
            return Vec::new();
        } else {
            panic!("Specified path does not exist. Check it/they exist(s): {path:?}");
        }
    };

    // Files referred to exactly are also ignored if matched, but we warn users
    // when that happens.
    let is_exact_file = metadata.is_file();

    let mut path_walk = if is_exact_file {
        let path = Path::new(&path);
        let dirpath = path.parent().unwrap().to_str().unwrap().to_string();
        let files = vec![path.file_name().unwrap().to_str().unwrap().to_string()];
        vec![(dirpath, None, files)]
    } else {
        let walkdir = WalkDir::new(&path);
        let entries: Vec<_> = if let Some(ignorer) = ignorer {
            // Apply the ignorer during traversal to skip ignored directories entirely.
            walkdir
                .into_iter()
                .filter_entry(|entry| {
                    let should_ignore = ignorer(entry.path());
                    if should_ignore {
                        let path_type = if entry.file_type().is_dir() {
                            "directory"
                        } else {
                            "file"
                        };
                        log::debug!(
                            "Skipping {} '{}' during file discovery traversal",
                            path_type,
                            entry.path().display()
                        );
                    }
                    !should_ignore
                })
                .filter_map(Result::ok)
                .collect()
        } else {
            walkdir.into_iter().filter_map(Result::ok).collect()
        };

        // Group entries by directory to maintain the original data structure.
        let mut dir_files: HashMap<String, Vec<String>> = HashMap::new();

        for entry in entries {
            if entry.file_type().is_file() {
                let dirpath = entry.path().parent().unwrap().to_str().unwrap().to_string();
                let filename = entry.file_name().to_str().unwrap().to_string();
                dir_files.entry(dirpath).or_default().push(filename);
            }
        }

        dir_files
            .into_iter()
            .map(|(dirpath, files)| (dirpath, None, files))
            .collect_vec()
    };

    // TODO: Discover ignore files between `path` and `working_path`.
    let ignore_file_paths: Vec<String> = Vec::new();

    // Add paths that could contain ignore files to the path walk.
    let path_walk_ignore_file: Vec<(String, Option<()>, Vec<String>)> = ignore_file_paths
        .iter()
        .map(|ignore_file_path| {
            let ignore_file_path = Path::new(ignore_file_path);
            let dir_name = ignore_file_path
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let file_name = vec![
                ignore_file_path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            ];

            (dir_name, None, file_name)
        })
        .collect();

    path_walk.extend(path_walk_ignore_file);

    let mut buffer = Vec::new();
    let mut ignores = HashMap::new();

    for (dirpath, _, filenames) in path_walk {
        for fname in filenames {
            let fpath = Path::new(&dirpath).join(&fname);

            if ignore_files && fname == ignore_file_name {
                let file = File::open(&fpath).unwrap();
                let lines = BufReader::new(file).lines();
                let spec = lines.map_while(Result::ok);
                ignores.insert(dirpath.clone(), spec.collect::<Vec<String>>());
                continue;
            }

            for ext in target_file_exts {
                if fname.to_lowercase().ends_with(ext) {
                    buffer.push(fpath.clone());
                }
            }
        }
    }

    let mut filtered_buffer = HashSet::new();

    for fpath in buffer {
        let npath = helpers::normalize(&fpath).to_str().unwrap().to_string();
        filtered_buffer.insert(npath);
    }

    let mut files = filtered_buffer.into_iter().collect_vec();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::paths_from_path;

    fn normalise_paths(paths: Vec<String>) -> Vec<String> {
        paths
            .into_iter()
            .map(|path| path.replace(['/', '\\'], "."))
            .collect()
    }

    #[test]
    fn test_linter_path_from_paths_dir() {
        let paths = paths_from_path(
            "test/fixtures/lexer".into(),
            None,
            None,
            None,
            None,
            &[".sql".into()],
            None,
        );
        let expected = vec![
            "test.fixtures.lexer.basic.sql",
            "test.fixtures.lexer.block_comment.sql",
            "test.fixtures.lexer.inline_comment.sql",
        ];
        assert_eq!(normalise_paths(paths), expected);
    }

    #[test]
    fn test_linter_path_from_paths_default() {
        let paths = normalise_paths(paths_from_path(
            "test/fixtures/linter".into(),
            None,
            None,
            None,
            None,
            &[".sql".into()],
            None,
        ));
        assert!(paths.contains(&"test.fixtures.linter.passing.sql".to_string()));
        assert!(paths.contains(&"test.fixtures.linter.passing_cap_extension.SQL".to_string()));
        assert!(!paths.contains(&"test.fixtures.linter.discovery_file.txt".to_string()));
    }

    #[test]
    fn test_linter_path_from_paths_exts() {
        let paths = normalise_paths(paths_from_path(
            "test/fixtures/linter".into(),
            None,
            None,
            None,
            None,
            &[".txt".into()],
            None,
        ));
        assert!(!paths.contains(&"test.fixtures.linter.passing.sql".into()));
        assert!(!paths.contains(&"test.fixtures.linter.passing_cap_extension.SQL".into()));
        assert!(paths.contains(&"test.fixtures.linter.discovery_file.txt".into()));
    }

    #[test]
    fn test_linter_path_from_paths_file() {
        let paths = paths_from_path(
            "test/fixtures/linter/indentation_errors.sql".into(),
            None,
            None,
            None,
            None,
            &[".sql".into()],
            None,
        );

        assert_eq!(
            normalise_paths(paths),
            &["test.fixtures.linter.indentation_errors.sql"]
        );
    }

    // test__linter__path_from_paths__not_exist
    // test__linter__path_from_paths__not_exist_ignore
    // test__linter__path_from_paths__explicit_ignore
    // test__linter__path_from_paths__sqlfluffignore_current_directory
    // test__linter__path_from_paths__dot
    // test__linter__path_from_paths__ignore
}
