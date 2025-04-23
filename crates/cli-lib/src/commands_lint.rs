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
    let LintArgs { paths, format } = args;
    let mut linter = linter(config, format, collect_parse_errors);

    linter.lint_paths(paths, false, &ignorer);

    linter.formatter().unwrap().completion_message();
    if linter.formatter().unwrap().has_fail() {
        1
    } else {
        0
    }
}

pub(crate) fn run_lint_stdin(
    config: FluffConfig,
    format: Format,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = linter(config, format, collect_parse_errors);
    linter.lint_string(&read_in, None, false);

    linter.formatter().unwrap().completion_message();

    if linter.formatter().unwrap().has_fail() {
        1
    } else {
        0
    }
}
