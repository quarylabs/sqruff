use crate::commands::{Format, LintArgs};
use crate::formatters::gitlab::GitlabFormatter;
use crate::linter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use std::path::Path;
use std::sync::Arc;

fn lint_linter(
    config: FluffConfig,
    args: &LintArgs,
    collect_parse_errors: bool,
) -> Result<Linter, String> {
    if matches!(args.format, Format::Gitlab) {
        let formatter = GitlabFormatter::new(args.annotation_level, args.write_output.as_deref())
            .map_err(|err| format!("Cannot open GitLab report: {err}"))?;
        Linter::new(
            config,
            Some(Arc::new(formatter)),
            None,
            collect_parse_errors,
        )
    } else if args.write_output.is_some() {
        Err("--write-output requires --format gitlab".into())
    } else {
        linter(config, args.format, collect_parse_errors, args.quiet, false)
    }
}

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let large_file_skip_fail = config
        .get("large_file_skip_fail", "core")
        .as_bool()
        .unwrap_or(false);
    let mut linter = match lint_linter(config, &args, collect_parse_errors) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let result = match linter.lint_paths_with_ignore_files(
        args.paths,
        false,
        &ignorer,
        !args.disregard_sqruffignores,
    ) {
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
    args: LintArgs,
    stdin_filename: Option<&Path>,
    ignorer: &(dyn Fn(&Path) -> bool + Send + Sync),
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = match lint_linter(config, &args, collect_parse_errors) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    if crate::stdin::stdin_filename_is_ignored(
        stdin_filename,
        !args.disregard_sqruffignores,
        linter.config().ignore_local_config(),
        ignorer,
    ) {
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
