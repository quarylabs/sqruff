use clap::Parser as _;
use sqruff_lib::api::ParseErrors;
use sqruff_lib::config::{ConfigInput, ConfigLoadOptions, ConfigLoader, ConfigOverrides};
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
mod logger;
mod reporters;
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
    let parse_errors = if cli.parsing_errors {
        ParseErrors::Include
    } else {
        ParseErrors::Suppress
    };

    let input = if let Some(config) = cli.config.as_ref() {
        if !config.is_file() {
            eprintln!(
                "The specified config file '{}' does not exist.",
                config.display()
            );

            return 1;
        };

        ConfigInput::File(config.clone())
    } else {
        ConfigInput::ProjectRoot(std::env::current_dir().unwrap_or_else(|_| ".".into()))
    };

    let config = match ConfigLoader::new().load(ConfigLoadOptions {
        input,
        ignore_local_config: false,
        overrides: ConfigOverrides {
            dialect: cli.dialect,
            ..Default::default()
        },
    }) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{}", error.message());
            return 1;
        }
    };

    match cli.command {
        Commands::Lint(args) => match is_std_in_flag_input(&args.paths) {
            Err(e) => {
                eprintln!("{e}");
                1
            }
            Ok(false) => commands_lint::run_lint(args, config, parse_errors),
            Ok(true) => commands_lint::run_lint_stdin(config, args.format, parse_errors),
        },
        Commands::Fix(args) => match is_std_in_flag_input(&args.paths) {
            Err(e) => {
                eprintln!("{e}");
                1
            }
            Ok(false) => commands_fix::run_fix(args, config, parse_errors),
            Ok(true) => commands_fix::run_fix_stdin(config, args.format, parse_errors),
        },
        Commands::Lsp => {
            if let Err(e) = sqruff_lsp::run() {
                eprintln!("LSP error: {e}");
                return 1;
            }
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
