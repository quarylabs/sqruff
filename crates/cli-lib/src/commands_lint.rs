use crate::commands::{Format, LintArgs};
use crate::linter;
use sqruff_lib::core::config::FluffConfig;
use std::path::Path;

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let LintArgs {
        paths,
        format,
        quiet,
        verbose: _,
        disregard_sqruffignores,
    } = args;
    let large_file_skip_fail = config
        .get("large_file_skip_fail", "core")
        .as_bool()
        .unwrap_or(false);
    let mut linter = match linter(config, format, collect_parse_errors, quiet, false) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let result =
        match linter.lint_paths_with_ignore_files(paths, false, &ignorer, !disregard_sqruffignores)
        {
            Ok(result) => result,
            Err(e) => {
                eprintln!("{}", e.value);
                return 1;
            }
        };

    linter.formatter().unwrap().completion_message(result.len());

    (result.has_violations() || (large_file_skip_fail && result.files_skipped() > 0)) as i32
}

pub(crate) fn run_lint_stdin(
    config: FluffConfig,
    format: Format,
    quiet: bool,
    stdin_filename: Option<&Path>,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
    disregard_ignores: bool,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = match linter(config, format, collect_parse_errors, quiet, false) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    if crate::stdin::stdin_filename_is_ignored(stdin_filename, !disregard_ignores, ignorer) {
        linter.formatter().unwrap().completion_message(0);
        return 0;
    }
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
