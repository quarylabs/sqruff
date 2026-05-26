use clap::Parser as _;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib_core::dialects::init::DialectKind;
use std::path::Path;
use std::sync::Arc;
use stdin::is_std_in_flag_input;

use crate::commands::{Cli, Commands};
#[cfg(feature = "codegen-docs")]
use crate::docs::codegen_docs;
pub mod commands;
mod commands_dialects;
mod commands_fix;
mod commands_info;
mod commands_lint;
#[cfg(feature = "parser")]
mod commands_parse;
mod commands_rules;
mod commands_templaters;
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
        match FluffConfig::try_from_file(Path::new(config)) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    } else {
        match FluffConfig::from_root(None, false, None) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
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
        Commands::Dialects => {
            commands_dialects::dialects();
            0
        }
        Commands::Templaters => {
            commands_templaters::templaters();
            0
        }
        #[cfg(feature = "parser")]
        Commands::Parse(args) => commands_parse::run_parse(args, config),
    }
}
