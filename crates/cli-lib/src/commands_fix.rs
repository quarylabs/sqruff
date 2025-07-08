use crate::commands::FixArgs;
use crate::commands::Format;
use crate::linter;
use sqruff_lib::core::config::FluffConfig;
use std::path::Path;

pub(crate) fn run_fix(
    args: FixArgs,
    config: FluffConfig,
    ignorer: impl Fn(&Path) -> bool + Send + Sync,
    collect_parse_errors: bool,
) -> i32 {
    let FixArgs { paths, format } = args;
    let mut linter = linter(config, format, collect_parse_errors);
    let result = linter.lint_paths(paths, true, &ignorer);

    if !result.has_violations() {
        println!("{} files processed, nothing to fix.", result.len());
        0
    } else {
        let any_unfixable_errors = result.has_unfixable_violations();
        let files = result.len();

        for mut file in result {
            let path = std::mem::take(&mut file.path);
            let fixed = file.fix_string();

            std::fs::write(path, fixed).unwrap();
        }

        linter.formatter_mut().unwrap().completion_message(files);

        any_unfixable_errors as i32
    }
}

pub(crate) fn run_fix_stdin(
    config: FluffConfig,
    format: Format,
    collect_parse_errors: bool,
) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = linter(config, format, collect_parse_errors);
    let result = linter.lint_string(&read_in, None, true);

    let has_unfixable_errors = result.has_unfixable_violations();

    println!("{}", result.fix_string());

    // if all fixable violations are fixable, return 0 else return 1
    has_unfixable_errors as i32
}
