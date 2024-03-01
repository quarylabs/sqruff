use std::process::exit;

use clap::Parser;
use glob::{glob, Paths};
use sqruff_lib::api::simple::{lint, lint_with_formatter};
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::rules::layout;

use crate::commands::Cli;

mod commands;

fn main() {
    match main_wrapper() {
        Ok(msg) => {
            println!("{}", msg);
            exit(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            exit(1)
        }
    }
}

// TODO Handle the unwraps better
fn main_wrapper() -> Result<String, String> {
    let cli = Cli::parse();
    let mut has_errors = false;

    let config = FluffConfig::from_root(None, false, None).unwrap();

    match cli.command {
        commands::Commands::Lint(lint_args) => {
            let files = find_files(&lint_args.file_path)?;

            let mut count = 0;
            for file in files {
                let formatter = OutputStreamFormatter::new(Box::new(std::io::stderr()), false);

                let file = file.unwrap();
                let file = file.to_str().unwrap();
                let contents = std::fs::read_to_string(file).unwrap();
                let linted = lint_with_formatter(
                    contents,
                    // TODO Make this a pointer
                    DEFAULT_DIALECT.to_string(),
                    layout::get_rules().into(),
                    None,
                    None,
                    formatter.into(),
                )
                .map_err(|e| format!("Error linting file '{}': {:?}", file, e))?;
                if !linted.is_empty() {
                    has_errors = true;
                }
                count += 1;
            }
        }
        commands::Commands::Fix(_) => {
            unimplemented!();
        }
    };

    if !has_errors { Ok(String::new()) } else { Err(String::new()) }
}

const DEFAULT_DIALECT: &str = "ansi";

fn find_files(pattern: &str) -> Result<Paths, String> {
    glob(pattern).map_err(|e| format!("Error finding files with pattern '{}': {:?}", pattern, e))
}
