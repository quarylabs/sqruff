use std::path::PathBuf;
use strum_macros::Display;

use clap::{Parser, Subcommand, ValueEnum};

use crate::github_action::is_in_github_action;

#[derive(Debug, Parser)]
#[command(name = "sqruff")]
#[command(about = "sqruff is a sql formatter and linter", long_about = None, version=env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Path to a configuration file.
    #[arg(long, global = true)]
    pub config: Option<String>,
    /// Override the dialect (e.g., bigquery, clickhouse, ansi).
    #[arg(long, global = true)]
    pub dialect: Option<String>,
    /// Show parse errors.
    #[arg(long, global = true, default_value = "false")]
    pub parsing_errors: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        name = "lint",
        about = "Lint SQL files via passing a list of files or using stdin"
    )]
    Lint(LintArgs),
    #[command(
        name = "fix",
        about = "Fix SQL files via passing a list of files or using stdin"
    )]
    Fix(FixArgs),
    #[command(name = "lsp", about = "Run an LSP server")]
    Lsp,
    #[command(
        name = "info",
        about = "Print information about sqruff and the current environment"
    )]
    Info,
    #[command(name = "rules", about = "Explain the available rules")]
    Rules,
    #[cfg(feature = "parser")]
    #[command(
        name = "parse",
        about = "Parse SQL and output the parse tree for debugging"
    )]
    Parse(ParseArgs),
}

#[derive(Debug, Parser)]
pub struct LintArgs {
    /// Files or directories to fix. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    #[arg(default_value_t, short, long)]
    pub format: Format,
}

#[derive(Debug, Parser)]
pub struct FixArgs {
    /// Files or directories to fix. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    /// The output format for the results.
    #[arg(default_value_t, short, long)]
    pub format: Format,
}

#[derive(Debug, Parser)]
pub struct ParseArgs {
    /// Files or directories to parse. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    /// The output format for the parse tree.
    #[arg(default_value_t, short, long)]
    pub format: ParseFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum, Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Format {
    Human,
    GithubAnnotationNative,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum, Display, Default)]
#[strum(serialize_all = "kebab-case")]
pub enum ParseFormat {
    Json,
    #[default]
    Pretty,
}

impl Default for Format {
    fn default() -> Self {
        if is_in_github_action() {
            Format::GithubAnnotationNative
        } else {
            Format::Human
        }
    }
}
