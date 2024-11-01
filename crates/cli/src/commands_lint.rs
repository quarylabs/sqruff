use crate::commands::{Format, LintArgs};
use crate::linter;
use sqruff_lib::core::config::FluffConfig;
use std::path::Path;

pub(crate) fn run_lint(
    args: LintArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
) -> i32 {
    let LintArgs { paths, format } = args;
    let mut linter = linter(config, format);
    let result = linter.lint_paths(paths, false, &ignorer);
    let count: usize = result.paths.iter().map(|path| path.files.len()).sum();

    // TODO this should be cleaned up better
    if matches!(format, Format::GithubAnnotationNative) {
        for path in result.paths {
            for file in path.files {
                for violation in file.violations {
                    let line = format!(
                        "::error title=sqruff,file={},line={},col={}::{}: {}",
                        file.path,
                        violation.line_no,
                        violation.line_pos,
                        violation.rule.as_ref().unwrap().code,
                        violation.description
                    );
                    eprintln!("{line}");
                }
            }
        }
    }

    eprintln!("The linter processed {count} file(s).");
    linter.formatter_mut().unwrap().completion_message();
    if linter
        .formatter()
        .unwrap()
        .has_fail
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        1
    } else {
        0
    }
}

pub(crate) fn run_lint_stdin(config: FluffConfig, format: Format) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let mut linter = linter(config, format);
    let result = linter.lint_string(&read_in, None, false);

    linter.formatter_mut().unwrap().completion_message();

    if result.get_violations(None).is_empty() {
        0
    } else {
        1
    }
}
