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
    /// Output without ANSI color codes.
    #[arg(short = 'n', long, global = true, overrides_with = "color")]
    pub nocolor: bool,
    /// Enable color on terminals, overriding NO_COLOR.
    #[arg(long, global = true, overrides_with = "nocolor")]
    pub color: bool,
    /// Path to a configuration file.
    #[arg(long, global = true)]
    pub config: Option<String>,
    /// Override the dialect (e.g., bigquery, clickhouse, ansi).
    #[arg(long, global = true)]
    pub dialect: Option<String>,
    /// Override the `library_path` value for the jinja templater. Set this to
    /// 'none' to disable it entirely. This overrides any values set by users in
    /// configuration files or inline directives.
    #[arg(long, global = true)]
    pub library_path: Option<String>,
    /// Load configuration for stdin as if it came from this file.
    #[arg(long, global = true)]
    pub stdin_filename: Option<PathBuf>,
    /// Show parse errors.
    #[arg(long, global = true, default_value = "false")]
    pub parsing_errors: bool,
    /// Ignore all but the listed rules in inline `noqa` comments.
    #[arg(long, global = true)]
    pub disable_noqa_except: Option<String>,
    /// Disable all inline `noqa` comments.
    #[arg(long, global = true, default_value = "false")]
    pub disable_noqa: bool,
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
    #[command(name = "dialects", about = "List available dialects")]
    Dialects,
    #[command(name = "templaters", about = "List available templaters")]
    Templaters,
    #[cfg(feature = "parser")]
    #[command(
        name = "parse",
        about = "Parse SQL and output the parse tree for debugging"
    )]
    Parse(ParseArgs),
}

#[derive(Debug, Parser)]
pub struct LintArgs {
    /// Perform the operation regardless of .sqruffignore and .sqlfluffignore configurations.
    #[arg(long, visible_alias = "disregard-sqlfluffignores")]
    pub disregard_sqruffignores: bool,

    /// Files or directories to fix. Use `-` to read from stdin.
    pub paths: Vec<PathBuf>,
    #[arg(default_value_t, short, long)]
    pub format: Format,
}

#[derive(Debug, Parser)]
pub struct FixArgs {
    /// Perform the operation regardless of .sqruffignore and .sqlfluffignore configurations.
    #[arg(long, visible_alias = "disregard-sqlfluffignores")]
    pub disregard_sqruffignores: bool,

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
    /// Include meta segments and source position information in JSON or YAML output.
    #[arg(long)]
    pub include_meta: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Format {
    Human,
    GithubAnnotationNative,
    Json,
    Sarif,
    /// Produce no output. Used mostly for testing.
    None,
}

#[derive(Debug, Clone, Copy, ValueEnum, Display, Default)]
#[strum(serialize_all = "kebab-case")]
pub enum ParseFormat {
    Json,
    Yaml,
    #[default]
    Pretty,
    /// Produce no output. Used mostly for testing.
    None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disable_noqa_except() {
        let cli =
            Cli::try_parse_from(["sqruff", "lint", "--disable-noqa-except", "CP01", "-"]).unwrap();

        assert_eq!(cli.disable_noqa_except.as_deref(), Some("CP01"));
    }
}

#[cfg(test)]
mod color_option_tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn color_flags_are_global_and_last_one_wins() {
        for (args, nocolor, color) in [
            (vec!["sqruff", "rules"], false, false),
            (vec!["sqruff", "-n", "rules"], true, false),
            (vec!["sqruff", "rules", "--nocolor"], true, false),
            (vec!["sqruff", "--color", "rules"], false, true),
            (vec!["sqruff", "rules", "--nocolor", "--color"], false, true),
            (vec!["sqruff", "rules", "--color", "-n"], true, false),
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!((cli.nocolor, cli.color), (nocolor, color));
        }
    }
}
