//! Discovery methods for SQL files.
//!
//! The main public method here is [`paths_from_path`], which takes a potentially
//! ambiguous path and resolves it into specific file references.

use std::path::{Path, PathBuf};

use hashbrown::HashSet;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use sqruff_lib_core::helpers;
use walkdir::WalkDir;

#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
use crate::core::config::{ConfigLoader, Value};

const CONFIG_IGNORE_FILE_NAMES: [&str; 2] = ["pyproject.toml", ".sqlfluff"];

#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
struct IgnoreSpecRecord {
    root: PathBuf,
    source: PathBuf,
    matcher: Gitignore,
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
struct IgnoreSpecRecord;

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        helpers::normalize(path)
    } else {
        helpers::normalize(&std::env::current_dir().unwrap().join(path))
    }
}

fn config_search_directories(target_path: &Path, working_path: &Path) -> Vec<PathBuf> {
    let target_dir = if target_path.is_file() {
        target_path.parent().unwrap_or(target_path)
    } else {
        target_path
    };

    let Some(common_path) = target_dir
        .ancestors()
        .find(|candidate| working_path.starts_with(candidate))
    else {
        return vec![working_path.to_path_buf(), target_dir.to_path_buf()];
    };

    let mut directories = Vec::new();
    let mut current = Some(target_dir);
    while let Some(directory) = current {
        directories.push(directory.to_path_buf());
        if directory == common_path {
            break;
        }
        current = directory.parent();
    }
    directories.reverse();
    directories
}

#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn load_ignorefile(path: &Path) -> Option<IgnoreSpecRecord> {
    let (matcher, error) = Gitignore::new(path);
    if let Some(error) = error {
        log::warn!(
            "Unable to fully load ignore file {}: {error}",
            path.display()
        );
    }

    Some(IgnoreSpecRecord {
        root: path.parent()?.to_path_buf(),
        source: path.to_path_buf(),
        matcher,
    })
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn load_ignorefile(_path: &Path) -> Option<IgnoreSpecRecord> {
    None
}

#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn load_configfile(path: &Path) -> Option<IgnoreSpecRecord> {
    let mut config = hashbrown::HashMap::new();
    if let Err(error) = ConfigLoader::try_load_config_file(path, &mut config) {
        log::warn!(
            "Unable to load ignore patterns from {}: {error}",
            path.display()
        );
        return None;
    }

    let patterns = config.get("core")?.as_map()?.get("ignore_paths")?;
    let patterns = match patterns {
        Value::String(patterns) => patterns.split(',').collect::<Vec<_>>(),
        Value::Array(patterns) if !patterns.is_empty() => patterns
            .iter()
            .map(Value::as_string)
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };

    let root = path.parent()?;
    let mut builder = GitignoreBuilder::new(root);
    for pattern in patterns {
        if let Err(error) = builder.add_line(Some(path.to_path_buf()), pattern) {
            log::warn!(
                "Unable to load ignore pattern from {}: {error}",
                path.display()
            );
        }
    }
    let matcher = match builder.build() {
        Ok(matcher) => matcher,
        Err(error) => {
            log::warn!(
                "Unable to build ignore patterns from {}: {error}",
                path.display()
            );
            return None;
        }
    };

    Some(IgnoreSpecRecord {
        root: root.to_path_buf(),
        source: path.to_path_buf(),
        matcher,
    })
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn load_configfile(_path: &Path) -> Option<IgnoreSpecRecord> {
    None
}

#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn check_ignore_specs<'a>(
    absolute_path: &Path,
    is_dir: bool,
    ignore_specs: &'a [IgnoreSpecRecord],
) -> Option<&'a Path> {
    for record in ignore_specs {
        if absolute_path.starts_with(&record.root)
            && record
                .matcher
                .matched_path_or_any_parents(absolute_path, is_dir)
                .is_ignore()
        {
            return Some(&record.source);
        }
    }
    None
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn check_ignore_specs<'a>(
    _absolute_path: &Path,
    _is_dir: bool,
    _ignore_specs: &'a [IgnoreSpecRecord],
) -> Option<&'a Path> {
    None
}

fn load_ignore_specs(
    path: &Path,
    working_path: &Path,
    ignore_file_name: &str,
    ignorer: Option<&(dyn Fn(&Path) -> bool + Send + Sync)>,
) -> Vec<IgnoreSpecRecord> {
    let absolute_target = absolute_path(path);
    let absolute_working_path = absolute_path(working_path);
    let mut ignore_paths = Vec::new();
    let mut ignore_file_names = vec![ignore_file_name];
    for config_file_name in &CONFIG_IGNORE_FILE_NAMES {
        if !ignore_file_names.contains(config_file_name) {
            ignore_file_names.push(config_file_name);
        }
    }

    for search_path in config_search_directories(&absolute_target, &absolute_working_path) {
        for file_name in &ignore_file_names {
            let candidate = search_path.join(file_name);
            if candidate.is_file() {
                ignore_paths.push(candidate);
            }
        }
    }

    if path.is_dir() {
        let entries = WalkDir::new(path)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| match ignorer {
                Some(ignorer) => !ignorer(entry.path()),
                None => true,
            })
            .filter_map(Result::ok);

        for entry in entries {
            if entry.file_type().is_file()
                && ignore_file_names
                    .iter()
                    .any(|file_name| entry.file_name() == *file_name)
            {
                ignore_paths.push(absolute_path(entry.path()));
            }
        }
    }

    let mut seen = HashSet::new();
    ignore_paths.retain(|path| seen.insert(path.clone()));
    ignore_paths.sort_by_key(|path| path.components().count());
    ignore_paths
        .iter()
        .filter_map(|path| {
            if path
                .file_name()
                .is_some_and(|name| name == ignore_file_name)
            {
                load_ignorefile(path)
            } else {
                load_configfile(path)
            }
        })
        .collect()
}

fn matches_file_extension(path: &Path, valid_extensions: &[String]) -> bool {
    let lowercase_path = path.to_string_lossy().to_lowercase();
    valid_extensions
        .iter()
        .any(|extension| lowercase_path.ends_with(extension))
}

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
    let working_path = PathBuf::from(
        working_path.unwrap_or_else(|| std::env::current_dir().unwrap().display().to_string()),
    );
    let lower_file_exts = target_file_exts
        .iter()
        .map(|extension| extension.to_lowercase())
        .collect::<Vec<_>>();

    let Ok(metadata) = std::fs::metadata(&path) else {
        if ignore_non_existent_files {
            return Vec::new();
        } else {
            panic!("Specified path does not exist. Check it/they exist(s): {path:?}");
        }
    };

    let ignore_specs = if ignore_files {
        load_ignore_specs(&path, &working_path, &ignore_file_name, ignorer)
    } else {
        Vec::new()
    };

    if metadata.is_file() {
        if !matches_file_extension(&path, &lower_file_exts) {
            return Vec::new();
        }
        if ignorer.is_some_and(|ignorer| ignorer(&path)) {
            return Vec::new();
        }

        let absolute_file = absolute_path(&path);
        if let Some(ignore_file) = check_ignore_specs(&absolute_file, false, &ignore_specs) {
            let display_ignore_file = ignore_file
                .strip_prefix(absolute_path(&working_path))
                .unwrap_or(ignore_file);
            log::warn!(
                "Exact file path {} was given but it was ignored by a pattern in {}; re-run with ignore files disabled to process it",
                path.display(),
                display_ignore_file.display()
            );
            return Vec::new();
        }

        return vec![helpers::normalize(&path).to_string_lossy().to_string()];
    }

    let mut files = WalkDir::new(&path)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            let externally_ignored = ignorer.is_some_and(|ignorer| ignorer(entry.path()));
            let internally_ignored = check_ignore_specs(
                &absolute_path(entry.path()),
                entry.file_type().is_dir(),
                &ignore_specs,
            )
            .is_some();

            if externally_ignored || internally_ignored {
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
                false
            } else {
                true
            }
        })
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file() && matches_file_extension(entry.path(), &lower_file_exts)
        })
        .map(|entry| {
            helpers::normalize(entry.path())
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
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

    #[test]
    fn test_linter_path_from_paths_specific_bad_ext() {
        let paths = paths_from_path(
            "test/fixtures/linter/sqlfluffignore/.sqlfluffignore".into(),
            None,
            None,
            None,
            None,
            &[".sql".into()],
            None,
        );
        assert!(paths.is_empty());
    }

    #[test]
    fn test_linter_path_from_paths_explicit_ignore() {
        let paths = paths_from_path(
            "test/fixtures/linter/sqlfluffignore/path_a/query_a.sql".into(),
            None,
            Some(true),
            Some(true),
            Some("test/fixtures/linter/sqlfluffignore".into()),
            &[".sql".into()],
            None,
        );
        assert!(paths.is_empty());
    }

    #[test]
    fn test_linter_path_from_paths_nested_ignore_files() {
        let paths = normalise_paths(paths_from_path(
            "test/fixtures/linter/sqlfluffignore".into(),
            None,
            None,
            None,
            None,
            &[".sql".into()],
            None,
        ));
        assert_eq!(
            paths,
            &["test.fixtures.linter.sqlfluffignore.path_b.query_b.sql"]
        );
    }

    #[test]
    fn test_linter_path_from_paths_ignore_files_disabled() {
        let paths = normalise_paths(paths_from_path(
            "test/fixtures/linter/sqlfluffignore".into(),
            None,
            None,
            Some(false),
            None,
            &[".sql".into()],
            None,
        ));
        assert_eq!(
            paths,
            &[
                "test.fixtures.linter.sqlfluffignore.path_a.query_a.sql",
                "test.fixtures.linter.sqlfluffignore.path_b.query_b.sql",
                "test.fixtures.linter.sqlfluffignore.path_b.query_c.sql",
                "test.fixtures.linter.sqlfluffignore.path_b.query_d.sql",
                "test.fixtures.linter.sqlfluffignore.path_c.query_e.sql",
            ]
        );
    }

    // test__linter__path_from_paths__not_exist
    // test__linter__path_from_paths__not_exist_ignore
    // test__linter__path_from_paths__dot
}
