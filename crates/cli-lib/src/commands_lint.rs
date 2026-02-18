use crate::commands::{Format, LintArgs};
use crate::linter;
use sqruff_lib::core::config::{ConfigLoader, FluffConfig};
use sqruff_lib::core::linter::linting_result::LintingResult;
use sqruff_lib_core::dialects::init::DialectKind;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Build a FluffConfig for files whose nearest config directory is `config_dir`.
/// Falls back to `base_config` when `config_dir` is None.
/// Applies `dialect_override` on top if provided.
fn config_for_group(
    config_dir: Option<&Path>,
    base_config: &FluffConfig,
    dialect_override: Option<DialectKind>,
) -> FluffConfig {
    let mut config = match config_dir {
        Some(dir) => FluffConfig::from_path(dir, None, false, None)
            .unwrap_or_else(|_| base_config.clone()),
        None => base_config.clone(),
    };
    if let Some(dialect) = dialect_override {
        // Unwrap is safe: dialect was already validated in the CLI.
        config.override_dialect(dialect).unwrap();
    }
    config
}

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
    dialect_override: Option<DialectKind>,
) -> i32 {
    let LintArgs { paths, format } = args;
    let mut linter = linter(config.clone(), format, collect_parse_errors);

    let result = lint_paths_with_per_file_config(
        &mut linter,
        paths,
        false,
        &ignorer,
        &config,
        dialect_override,
    );
    let result = match result {
        Ok(result) => result,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };

    linter.formatter().unwrap().completion_message(result.len());

    result.has_violations() as i32
}

pub(crate) fn run_lint_stdin(
    config: FluffConfig,
    format: Format,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = linter(config, format, collect_parse_errors);
    let result = match linter.lint_string(&read_in, None, false) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("{}", e.value);
            return 1;
        }
    };

    linter.formatter().unwrap().completion_message(1);

    result.has_violations() as i32
}

/// Expand paths, group files by nearest config directory, and lint each group
/// with the appropriate config. When `dialect_override` is None (e.g. because
/// --config was given explicitly), per-file config resolution is skipped.
pub(crate) fn lint_paths_with_per_file_config(
    linter: &mut sqruff_lib::core::linter::core::Linter,
    paths: Vec<PathBuf>,
    fix: bool,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
    base_config: &FluffConfig,
    dialect_override: Option<DialectKind>,
) -> Result<LintingResult, sqruff_lib_core::errors::SQLFluffUserError> {
    // Expand directories to individual files.
    let mut expanded: Vec<PathBuf> = Vec::new();
    let input_paths = if paths.is_empty() {
        vec![std::env::current_dir().unwrap()]
    } else {
        paths
    };
    for path in input_paths {
        if path.is_file() {
            expanded.push(path);
        } else {
            for p in linter.paths_from_path(path, None, None, None, None, Some(ignorer)) {
                expanded.push(PathBuf::from(p));
            }
        }
    }

    let expanded: Vec<PathBuf> = expanded
        .into_iter()
        .filter(|path| {
            let should_ignore = ignorer(path);
            if should_ignore {
                log::debug!(
                    "Filtering out ignored file '{}' from final processing list",
                    path.display()
                );
            }
            !should_ignore
        })
        .collect();

    if expanded.is_empty() {
        return Ok(LintingResult::new(Vec::new()));
    }

    if dialect_override.is_some() {
        // Group files by nearest config directory.
        let mut groups: HashMap<Option<PathBuf>, Vec<PathBuf>> = HashMap::new();
        for path in expanded {
            let config_dir = ConfigLoader::find_nearest_config_dir(&path);
            groups.entry(config_dir).or_default().push(path);
        }

        let mut all_files = Vec::new();
        for (config_dir, group_paths) in groups {
            let config =
                config_for_group(config_dir.as_deref(), base_config, dialect_override);
            linter.set_config(config);
            let result = linter.lint_paths(group_paths, fix, ignorer)?;
            all_files.extend(result);
        }
        Ok(LintingResult::new(all_files))
    } else {
        // No per-file resolution (explicit --config or no --dialect).
        linter.lint_paths(expanded, fix, ignorer)
    }
}
