use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ignore::gitignore::Gitignore;
use sqruff_lib_core::errors::SQLFluffUserError;
use sqruff_lib_core::helpers;

use super::{RunReport, Source, SourceId, SqruffError};

const DEFAULT_IGNORE_FILE_NAME: &str = ".sqruffignore";
const DEFAULT_SQL_FILE_EXTS: &[&str] = &[".sql"];

pub trait IgnoreMatcher: Send + Sync {
    fn is_ignored(&self, path: &Path) -> bool;
}

impl<F> IgnoreMatcher for F
where
    F: Fn(&Path) -> bool + Send + Sync + ?Sized,
{
    fn is_ignored(&self, path: &Path) -> bool {
        self(path)
    }
}

pub struct IgnoreFile {
    ignore: Gitignore,
}

impl IgnoreFile {
    pub fn from_root(root: &Path) -> Result<Self, SqruffError> {
        Self::from_root_with_name(root, DEFAULT_IGNORE_FILE_NAME)
    }

    pub fn from_root_with_name(root: &Path, ignore_file_name: &str) -> Result<Self, SqruffError> {
        let ignore_path = root.join(ignore_file_name);
        if !ignore_path.exists() {
            return Ok(Self {
                ignore: Gitignore::empty(),
            });
        }

        let (ignore, err) = Gitignore::new(ignore_path);
        if let Some(err) = err {
            return Err(SQLFluffUserError::new(err.to_string()));
        }

        Ok(Self { ignore })
    }
}

impl IgnoreMatcher for IgnoreFile {
    fn is_ignored(&self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        let match_result = self.ignore.matched(path, is_dir);
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

pub struct Workspace {
    pub root: PathBuf,
    pub ignore_file: IgnoreFile,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Result<Self, SqruffError> {
        let ignore_file = IgnoreFile::from_root(&root)?;
        Ok(Self { root, ignore_file })
    }

    pub fn discover_sources(
        &self,
        paths: &[PathBuf],
        options: &PathDiscoveryOptions<'_>,
    ) -> Result<Vec<Source<'static>>, SqruffError> {
        let effective_ignorer = options.ignorer.unwrap_or(&self.ignore_file);
        let options = PathDiscoveryOptions {
            ignore_file_name: options.ignore_file_name,
            ignore_non_existent_files: options.ignore_non_existent_files,
            ignore_files: options.ignore_files,
            working_dir: options.working_dir.clone(),
            ignorer: Some(effective_ignorer),
        };
        let mut sources = Vec::new();
        let paths = if paths.is_empty() {
            vec![self.root.clone()]
        } else {
            paths.to_vec()
        };

        for path in paths {
            if path.is_file() {
                sources.push(source_from_path(path)?);
                continue;
            }

            for path in discover_paths(&path, &options)? {
                sources.push(source_from_path(path)?);
            }
        }

        Ok(sources)
    }

    pub fn apply_fixes(&self, report: &RunReport) -> Result<(), SqruffError> {
        for file in &report.files {
            if file
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.is_none())
            {
                continue;
            }

            let Some(fixed_source) = &file.fixed_source else {
                continue;
            };

            let SourceId::Path(path) = &file.source_id else {
                continue;
            };

            if std::fs::read_to_string(path).is_ok_and(|current| current == *fixed_source) {
                continue;
            }

            std::fs::write(path, fixed_source).map_err(|err| {
                SQLFluffUserError::new(format!("Failed to write '{}': {err}", path.display()))
            })?;
        }

        Ok(())
    }
}

pub struct PathDiscoveryOptions<'a> {
    pub ignore_file_name: &'a str,
    pub ignore_non_existent_files: bool,
    pub ignore_files: bool,
    pub working_dir: PathBuf,
    pub ignorer: Option<&'a dyn IgnoreMatcher>,
}

impl<'a> PathDiscoveryOptions<'a> {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            ignore_file_name: DEFAULT_IGNORE_FILE_NAME,
            ignore_non_existent_files: false,
            ignore_files: true,
            working_dir,
            ignorer: None,
        }
    }
}

pub fn discover_paths(
    path: &Path,
    options: &PathDiscoveryOptions<'_>,
) -> Result<Vec<PathBuf>, SqruffError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        options.working_dir.join(path)
    };

    let Ok(metadata) = std::fs::metadata(&path) else {
        if options.ignore_non_existent_files {
            return Ok(Vec::new());
        }
        return Err(SQLFluffUserError::new(format!(
            "Specified path does not exist. Check it/they exist(s): {path:?}"
        )));
    };

    if metadata.is_file() {
        return Ok(vec![helpers::normalize(&path)]);
    }

    let mut paths = BTreeSet::new();
    let ignore_file = if options.ignore_files {
        Some(IgnoreFile::from_root_with_name(
            &options.working_dir,
            options.ignore_file_name,
        )?)
    } else {
        None
    };
    let fallback_ignorer = ignore_file
        .as_ref()
        .map(|ignore_file| ignore_file as &dyn IgnoreMatcher);
    collect_paths(&path, options, fallback_ignorer, &mut paths)?;
    Ok(paths.into_iter().collect())
}

fn collect_paths(
    dir: &Path,
    options: &PathDiscoveryOptions<'_>,
    fallback_ignorer: Option<&dyn IgnoreMatcher>,
    paths: &mut BTreeSet<PathBuf>,
) -> Result<(), SqruffError> {
    if is_ignored(dir, options, fallback_ignorer) {
        log::debug!(
            "Skipping directory '{}' during file discovery traversal",
            dir.display()
        );
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|err| {
        SQLFluffUserError::new(format!(
            "Failed to read directory '{}': {err}",
            dir.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            SQLFluffUserError::new(format!(
                "Failed to read directory '{}': {err}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            SQLFluffUserError::new(format!("Failed to inspect '{}': {err}", path.display()))
        })?;

        if file_type.is_dir() {
            collect_paths(&path, options, fallback_ignorer, paths)?;
        } else if file_type.is_file()
            && is_lintable_file(&path)
            && !is_ignored(&path, options, fallback_ignorer)
        {
            paths.insert(helpers::normalize(&path));
        }
    }

    Ok(())
}

fn is_ignored(
    path: &Path,
    options: &PathDiscoveryOptions<'_>,
    fallback_ignorer: Option<&dyn IgnoreMatcher>,
) -> bool {
    options
        .ignorer
        .is_some_and(|ignorer| ignorer.is_ignored(path))
        || fallback_ignorer.is_some_and(|ignorer| ignorer.is_ignored(path))
}

fn is_lintable_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let file_name = file_name.to_lowercase();

    DEFAULT_SQL_FILE_EXTS
        .iter()
        .any(|ext| file_name.ends_with(ext))
}

fn source_from_path(path: PathBuf) -> Result<Source<'static>, SqruffError> {
    let text = std::fs::read_to_string(&path).map_err(|err| {
        SQLFluffUserError::new(format!("Failed to read '{}': {err}", path.display()))
    })?;

    Ok(Source {
        id: SourceId::Path(path),
        text: Cow::Owned(text),
    })
}
