use clap::Parser as _;
use commands::Format;
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::core::Linter;
use std::path::Path;
use std::sync::Arc;
use stdin::is_std_in_flag_input;

use crate::commands::{Cli, Commands};
#[cfg(feature = "codegen-docs")]
use crate::docs::codegen_docs;

mod commands;
mod commands_fix;
mod commands_lint;
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

    let status_code = match cli.command {
        Commands::Lint(args) => match is_std_in_flag_input(&args.paths) {
            Err(e) => {
                eprintln!("{e}");
                1
            }
            Ok(false) => commands_lint::run_lint(args, config, ignorer),
            Ok(true) => commands_lint::run_lint_stdin(config, args.format),
        },
        Commands::Fix(args) => match is_std_in_flag_input(&args.paths) {
            Err(e) => {
                eprintln!("{e}");
                1
            }
            Ok(false) => commands_fix::run_fix(args, config, ignorer),
            Ok(true) => commands_fix::run_fix_stdin(config, args.format),
        },
        Commands::Lsp => {
            sqruff_lsp::run();
            0
        }
    };

    std::process::exit(status_code);
}

pub(crate) fn linter(config: FluffConfig, format: Format) -> Linter {
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
