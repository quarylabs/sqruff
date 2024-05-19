use clap::Parser as _;
use commands::{FixArgs, Format, LintArgs};
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;

use crate::commands::{Cli, Commands};

mod commands;

#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let cli = Cli::parse();

    let config = FluffConfig::from_root(None, false, None).unwrap();
    match cli.command {
        Commands::Lint(LintArgs { paths, format }) => {
            let mut linter = linter(config, format);
            let result = linter.lint_paths(paths, false);
            let count: usize = result.paths.iter().map(|path| path.files.len()).sum();

            if let Format::GithubAnnotationNative = format {
                for path in result.paths {
                    for file in path.files {
                        for violation in file.violations {
                            let line = format!(
                                "::error title=SQLFluff,file={},line={},col={}::{}: {}",
                                file.path,
                                violation.line_no,
                                violation.line_pos,
                                violation.rule.as_ref().unwrap().code(),
                                violation.description
                            );
                            eprintln!("{line}");
                        }
                    }
                }
            }

            println!("The linter processed {count} file(s).");
            linter.formatter.as_mut().unwrap().completion_message();

            std::process::exit(
                if linter.formatter.unwrap().has_fail.load(std::sync::atomic::Ordering::SeqCst) {
                    1
                } else {
                    0
                },
            )
        }
        Commands::Fix(FixArgs { paths, force, format }) => {
            let mut linter = linter(config, format);
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
                        return;
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
}

fn linter(config: FluffConfig, format: Format) -> Linter {
    let output_stream: Box<dyn std::io::Write + Send + Sync> = match format {
        Format::Human => Box::new(std::io::stderr()),
        Format::GithubAnnotationNative => Box::new(std::io::sink()),
    };

    let formatter = OutputStreamFormatter::new(
        output_stream,
        config.get("nocolor", "core").as_bool().unwrap_or_default(),
    );

    Linter::new(config, formatter.into(), None)
}

fn check_user_input() -> Option<bool> {
    use std::io::Write;

    let mut term = console::Term::stdout();
    _ = term.write(b"Are you sure you wish to attempt to fix these? [Y/n] ").unwrap();
    term.flush().unwrap();

    let ret = match term.read_char().unwrap().to_ascii_lowercase() {
        'y' | '\r' | '\n' => Some(true),
        'n' => Some(false),
        _ => None,
    };
    _ = term.write(b" ...\n").unwrap();
    ret
}
