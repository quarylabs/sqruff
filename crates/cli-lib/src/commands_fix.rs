use crate::check_user_input;
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
    let FixArgs {
        paths,
        force,
        format,
    } = args;
    let mut linter = linter(config, format, collect_parse_errors);
    let result = linter.lint_paths(paths, true, &ignorer);

    if result.files.iter().all(|file| file.violations.is_empty()) {
        let count_files = result.files.len();
        println!("{} files processed, nothing to fix.", count_files);
        0
    } else {
        if !force {
            match check_user_input() {
                Some(true) => {
                    eprintln!("Attempting fixes...");
                }
                Some(false) => return 0,
                None => {
                    eprintln!("Invalid input, please enter 'Y' or 'N'");
                    eprintln!("Aborting...");
                    return 0;
                }
            }
        }

        let any_unfixable_errors = result
            .files
            .iter()
            .any(|file| !file.get_violations(Some(false)).is_empty());

        for mut file in result.files {
            let path = std::mem::take(&mut file.path);
            let write_buff = file.fix_string();
            std::fs::write(path, write_buff).unwrap();
        }

        linter.formatter_mut().unwrap().completion_message();

        if any_unfixable_errors { 1 } else { 0 }
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

    // print fixed to std out
    let violations = result.get_violations(Some(false));
    println!("{}", result.fix_string());

    // if all fixable violations are fixable, return 0 else return 1
    if violations.is_empty() { 0 } else { 1 }
}
