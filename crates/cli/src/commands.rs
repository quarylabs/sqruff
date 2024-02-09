use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "sqruff")]
#[command(about = "sqruff is a sql formatter and linter", long_about = None, version=env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "lint", about = "lint files")]
    Lint(LintArgs),
    #[command(name = "fix", about = "fix files")]
    Fix(FixArgs),
}

#[derive(Debug, Parser)]
pub struct LintArgs {
    /// glob pattern to lint
    pub file_path: String,
}

#[derive(Debug, Parser)]
pub struct FixArgs {
    /// glob pattern to fix
    pub file_path: String,
}
