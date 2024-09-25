use clap::Parser as _;
use commands::{FixArgs, Format, LintArgs};
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;

use crate::commands::{Cli, Commands};
#[cfg(feature = "codegen-docs")]
use crate::docs::codegen_docs;

mod commands;
#[cfg(feature = "codegen-docs")]
mod docs;
mod github_action;

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
    match cli.command {
        Commands::Lint(LintArgs { paths, format }) => {
            let mut linter = linter(config, format);
            let result = linter.lint_paths(paths, false);
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
        Commands::Fix(FixArgs {
            paths,
            force,
            format,
        }) => {
            let mut linter = linter(config, format);
            let result = linter.lint_paths(paths, true);

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
        Commands::Lsp => lsp::run(),
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
