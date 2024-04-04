use clap::Parser as _;
use commands::{FixArgs, LintArgs};
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;

use crate::commands::{Cli, Commands};

mod commands;

#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let config = FluffConfig::default();
    let formatter = OutputStreamFormatter::new(
        Box::new(std::io::stderr()),
        config.get("nocolor", "core").as_bool().unwrap_or_default(),
    );

    let cli = Cli::parse();
    let mut linter = Linter::new(config, formatter.into(), None);

    match cli.command {
        Commands::Lint(LintArgs { paths }) => {
            linter.lint_paths(paths, false);
        }
        Commands::Fix(FixArgs { paths, force }) => {
            let result = linter.lint_paths(paths, true);

            if !force {
                match check_user_input() {
                    Some(true) => {
                        println!("Attempting fixes...");
                    }
                    Some(false) => return,
                    None => {
                        println!("Invalid input, please enter 'Y' or 'N'");
                        println!("Aborting...");
                    }
                }
            }

            for linted_dir in result.paths {
                for file in linted_dir.files {
                    let write_buff = file.fix_string();
                    std::fs::write(file.path, write_buff).unwrap();
                }
            }

            linter.formatter.as_mut().unwrap().completion_message();
        }
    }

    std::process::exit(if linter.formatter.unwrap().has_fail.get() { 1 } else { 0 })
}

fn check_user_input() -> Option<bool> {
    use std::io::Write;

    let mut term = console::Term::stdout();
    term.write(b"Are you sure you wish to attempt to fix these? [Y/n] ").unwrap();
    term.flush().unwrap();

    let ret = match term.read_char().unwrap().to_ascii_lowercase() {
        'y' | '\r' | '\n' => Some(true),
        'n' => Some(false),
        _ => None,
    };
    term.write(b" ...\n").unwrap();
    ret
}
