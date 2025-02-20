use clap::Parser as _;
use commands::Format;
use sqruff_lib::cli::formatters::Formatter;
use sqruff_lib::cli::json::JsonFormatter;
use sqruff_lib::cli::{
    formatters::OutputStreamFormatter,
    github_annotation_native_formatter::GithubAnnotationNativeFormatter,
};
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
mod commands_info;
mod commands_lint;
mod commands_rules;
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
    let collect_parse_errors = cli.parsing_errors;

    let config: FluffConfig = if let Some(config) = cli.config.as_ref() {
        if !Path::new(config).is_file() {
            eprintln!(
                "The specified config file '{}' does not exist.",
                cli.config.as_ref().unwrap()
            );

            std::process::exit(1);
        };
        let read_file = std::fs::read_to_string(config).unwrap();
        FluffConfig::from_source(&read_file, None)
    } else {
        FluffConfig::from_root(None, false, None).unwrap()
    };

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
            Ok(false) => commands_lint::run_lint(args, config, ignorer, collect_parse_errors),
            Ok(true) => commands_lint::run_lint_stdin(config, args.format, collect_parse_errors),
        },
        Commands::Fix(args) => match is_std_in_flag_input(&args.paths) {
            Err(e) => {
                eprintln!("{e}");
                1
            }
            Ok(false) => commands_fix::run_fix(args, config, ignorer, collect_parse_errors),
            Ok(true) => commands_fix::run_fix_stdin(config, args.format, collect_parse_errors),
        },
        Commands::Lsp => {
            sqruff_lsp::run();
            0
        }
        Commands::Info => {
            commands_info::info();
            0
        }
        Commands::Rules => {
            commands_rules::rules_info(config);
            0
        }
    };

    std::process::exit(status_code);
}

pub(crate) fn linter(config: FluffConfig, format: Format, collect_parse_errors: bool) -> Linter {
    let formatter: Arc<dyn Formatter> = match format {
        Format::Human => {
            let output_stream = std::io::stderr().into();
            let formatter = OutputStreamFormatter::new(
                output_stream,
                config.get("nocolor", "core").as_bool().unwrap_or_default(),
                config.get("verbose", "core").as_int().unwrap_or_default(),
            );
            Arc::new(formatter)
        }
        Format::GithubAnnotationNative => {
            let output_stream = std::io::stderr();
            let formatter = GithubAnnotationNativeFormatter::new(output_stream);
            Arc::new(formatter)
        }
        Format::Json => {
            let formatter = JsonFormatter::default();
            Arc::new(formatter)
        }
    };

    Linter::new(config, Some(formatter), None, collect_parse_errors)
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
