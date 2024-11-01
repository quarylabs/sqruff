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
) -> i32 {
    let FixArgs {
        paths,
        force,
        format,
    } = args;
    let mut linter = linter(config, format);
    let result = linter.lint_paths(paths, true, &ignorer);

    if result
        .paths
        .iter()
        .map(|path| path.files.iter().all(|file| file.violations.is_empty()))
        .all(|v| v)
    {
        let count_files = result
            .paths
            .iter()
            .map(|path| path.files.len())
            .sum::<usize>();
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

        for linted_dir in result.paths {
            for mut file in linted_dir.files {
                let path = std::mem::take(&mut file.path);
                let write_buff = file.fix_string();
                std::fs::write(path, write_buff).unwrap();
            }
        }

        linter.formatter_mut().unwrap().completion_message();
        0
    }
}

pub(crate) fn run_fix_stdin(config: FluffConfig, format: Format) -> i32 {
    let read_in = crate::stdin::read_std_in().unwrap();

    let linter = linter(config, format);
    let result = linter.lint_string(&read_in, None, true);

    // print fixed to std out
    let violations = result.get_violations(Some(false));
    println!("{}", result.fix_string());

    // if all fixable violations are fixable, return 0 else return 1
    if violations.is_empty() {
        0
    } else {
        1
    }
}
