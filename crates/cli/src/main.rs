use clap::Parser as _;
use commands::LintArgs;
use sqruff_lib::cli::formatters::OutputStreamFormatter;
use sqruff_lib::core::config::FluffConfig;
use sqruff_lib::core::linter::linter::Linter;

use crate::commands::{Cli, Commands};

mod commands;

#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let config = FluffConfig::default();
    let formatter = OutputStreamFormatter::new(
        Box::new(std::io::stderr()),
        config.get("nocolor", "core").as_bool().unwrap_or_default(),
    );

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
