use clap::Parser as _;
use commands::Format;
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib::{Formatter, core::config::FluffConfig};
use sqruff_lib_core::dialects::init::DialectKind;
use std::path::Path;
use std::sync::Arc;
use stdin::is_std_in_flag_input;

use crate::commands::{Cli, Commands};
#[cfg(feature = "codegen-docs")]
use crate::docs::codegen_docs;
use crate::formatters::OutputStreamFormatter;
use crate::formatters::github_annotation_native_formatter::GithubAnnotationNativeFormatter;
use crate::formatters::json::JsonFormatter;

pub mod commands;
mod commands_fix;
mod commands_info;
mod commands_lint;
#[cfg(feature = "parser")]
mod commands_parse;
mod commands_rules;
#[cfg(feature = "codegen-docs")]
mod docs;
mod formatters;
mod github_action;
mod ignore;
mod logger;
mod stdin;

#[cfg(feature = "codegen-docs")]
pub fn run_docs_generation() {
    #[cfg(feature = "codegen-docs")]
    return codegen_docs();
}

pub fn run_with_args<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let _ = logger::init();
    let cli = Cli::parse_from(args);
    let collect_parse_errors = cli.parsing_errors;

    let mut config: FluffConfig = if let Some(config) = cli.config.as_ref() {
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

    if let Some(dialect) = cli.dialect {
        let dialect_kind = DialectKind::try_from(dialect.as_str());
        match dialect_kind {
            Ok(dialect_kind) => {
                config.override_dialect(dialect_kind).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    std::process::exit(1);
                });
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let current_path = std::env::current_dir().unwrap();
    let ignore_file = ignore::IgnoreFile::new_from_root(&current_path).unwrap();
    let ignore_file = Arc::new(ignore_file);
    let ignorer = {
        let ignore_file = Arc::clone(&ignore_file);
        move |path: &Path| ignore_file.is_ignored(path)
    };

    match cli.command {
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
        #[cfg(feature = "parser")]
        Commands::Parse(args) => commands_parse::run_parse(args, config),
    }
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
