use clap::Parser as _;
use commands::LintArgs;
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;

use crate::commands::{Cli, Commands};

mod commands;

fn main() {
    let formatter = OutputStreamFormatter::new(Box::new(std::io::stderr()), false);
    let config = FluffConfig::default();

    let cli = Cli::parse();
    let mut linter = Linter::new(config, formatter.into(), None);

    match cli.command {
        Commands::Lint(LintArgs { paths }) => {
            linter.lint_paths(paths);
        }
        Commands::Fix(_) => todo!(),
    }

    std::process::exit(if linter.formatter.unwrap().has_fail.get() { 1 } else { 0 })
}
