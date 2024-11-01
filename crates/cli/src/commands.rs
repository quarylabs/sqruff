use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::github_action::is_in_github_action;

#[derive(Debug, Parser)]
#[command(name = "sqruff")]
#[command(about = "sqruff is a sql formatter and linter", long_about = None, version=env!("CARGO_PKG_VERSION")
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Path to a configuration file.
    #[arg(long, global = true)]
    pub config: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    #[command(name = "lint", about = "lint files")]
    Lint(LintArgs),
    #[command(name = "fix", about = "fix files")]
    Fix(FixArgs),
    #[command(name = "lsp", about = "Run an LSP server")]
    Lsp,
}

#[derive(Debug, Parser)]
pub(crate) struct LintArgs {
    /// Files or directories to fix. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    #[arg(default_value = "human", short, long)]
    pub format: Format,
}

#[derive(Debug, Parser)]
pub(crate) struct FixArgs {
    /// Files or directories to fix. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    /// Skip the confirmation prompt and go straight to applying fixes.
    #[arg(long)]
    pub force: bool,
    #[arg(default_value = "human", short, long)]
    pub format: Format,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Format {
    Human,
    GithubAnnotationNative,
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
