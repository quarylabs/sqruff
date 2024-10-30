use clap::Parser as _;
use commands::{FixArgs, Format, LintArgs};
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use stdin::is_std_in_flag_input;
use std::path::Path;
use std::sync::Arc;
use std::io::Read;

use crate::commands::{Cli, Commands};
#[cfg(feature = "codegen-docs")]
use crate::docs::codegen_docs;

mod commands;
#[cfg(feature = "codegen-docs")]
mod docs;
mod github_action;
mod ignore;
mod stdin;

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "openbsd"),
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64"
    )
))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(target_os = "windows")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg_attr(feature = "codegen-docs", allow(unreachable_code))]
fn main() {
    #[cfg(feature = "codegen-docs")]
    return codegen_docs();

    let cli = Cli::parse();

    let mut extra_config_path = None;
    let mut ignore_local_config = false;

    if cli.config.is_some() {
        extra_config_path = cli.config;
        ignore_local_config = true;
    }

    let config = FluffConfig::from_root(extra_config_path, ignore_local_config, None).unwrap();
    let current_path = std::env::current_dir().unwrap();
    let ignore_file = ignore::IgnoreFile::new_from_root(&current_path).unwrap();
    let ignore_file = Arc::new(ignore_file);
    let ignorer = {
        let ignore_file = Arc::clone(&ignore_file);
        move |path: &Path| ignore_file.is_ignored(path)
    };

    match cli.command {
        Commands::Lint(LintArgs { paths, format }) => {
            let mut linter = linter(config, format);

            if is_std_in_flag_input(&paths) {
                // Read SQL input from stdin
                let mut sql_input = String::new();
                std::io::stdin().read_to_string(&mut sql_input).unwrap();
                // Lint the input SQL string
                let result = linter.lint_string_wrapped(&sql_input, None, false);

                if matches!(format, Format::GithubAnnotationNative) {
                    // Handle GithubAnnotationNative format
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
                } else {
                    // Use formatter to output results
                    linter.formatter_mut().unwrap().dispatch_linting_result(&result);
                }

                linter.formatter_mut().unwrap().completion_message();

                std::process::exit(
                    if linter
                        .formatter()
                        .unwrap()
                        .has_fail
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        1
                    } else {
                        0
                    },
                );
            } else {
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

                std::process::exit(
                    if linter
                        .formatter()
                        .unwrap()
                        .has_fail
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        1
                    } else {
                        0
                    },
                )
            }
        }
        Commands::Fix(FixArgs {
            paths,
            force,
            format,
        }) => {
            let mut linter = linter(config, format);

            if is_std_in_flag_input(&paths) {
                // Read SQL input from stdin
                let mut sql_input = String::new();
                std::io::stdin().read_to_string(&mut sql_input).unwrap();
                // Fix the input SQL string
                let result = linter.lint_string_wrapped(&sql_input, None, true);

                if !result
                    .paths
                    .iter()
                    .flat_map(|path| &path.files)
                    .all(|file| file.violations.is_empty())
                {
                    if !force {
                        match check_user_input() {
                            Some(true) => {
                                eprintln!("Attempting fixes...");
                            }
                            Some(false) => return,
                            None => {
                                eprintln!("Invalid input, please enter 'Y' or 'N'");
                                eprintln!("Aborting...");
                                return;
                            }
                        }
                    }
                }

                // Output the fixed SQL
                let fixed_sql = result.paths[0].files[0].fix_string();
                println!("{}", fixed_sql);
            } else {
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
                    return;
                }

                if !force {
                    match check_user_input() {
                        Some(true) => {
                            eprintln!("Attempting fixes...");
                        }
                        Some(false) => return,
                        None => {
                            eprintln!("Invalid input, please enter 'Y' or 'N'");
                            eprintln!("Aborting...");
                            return;
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
            }
        }
        Commands::Lsp => sqruff_lsp::run(),
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
    _ = term
        .write(b"Are you sure you wish to attempt to fix these? [Y/n] ")
        .unwrap();
    term.flush().unwrap();

    let ret = match term.read_char().unwrap().to_ascii_lowercase() {
        'y' | '\r' | '\n' => Some(true),
        'n' => Some(false),
        _ => None,
    };
    _ = term.write(b" ...\n").unwrap();
    ret
}
