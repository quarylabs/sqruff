use std::mem::take;
use std::str::FromStr;
use std::sync::Arc;

use ahash::AHashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::errors::{SQLBaseError, SQLFluffUserError};

use crate::cli::formatters::Formatter;
use crate::core::config::FluffConfig;
use crate::core::linter::core::Linter;

pub fn get_simple_config(
    dialect: Option<String>,
    rules: Option<Vec<String>>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<FluffConfig, SQLFluffUserError> {
    let mut overrides = AHashMap::new();
    if let Some(dialect) = dialect {
        DialectKind::from_str(dialect.as_str())
            .map_err(|error| SQLFluffUserError::new(error.to_string()))?;
        overrides.insert("dialect".to_owned(), dialect);
    }
    if let Some(rules) = rules {
        overrides.insert("rules".to_owned(), rules.join(","));
    }
    if let Some(exclude_rules) = exclude_rules {
        overrides.insert("exclude_rules".to_owned(), exclude_rules.join(","));
    }

    FluffConfig::from_root(config_path, true, Some(overrides))
        .map_err(|err| SQLFluffUserError::new(format!("Error loading config: {:?}", err)))
}

pub fn lint(
    sql: String,
    dialect: String,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<Vec<SQLBaseError>, SQLFluffUserError> {
    lint_with_formatter(&sql, dialect, exclude_rules, config_path, None)
}

/// Lint a SQL string.
pub fn lint_with_formatter(
    sql: &str,
    dialect: String,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
    formatter: Option<Arc<dyn Formatter>>,
) -> Result<Vec<SQLBaseError>, SQLFluffUserError> {
    let cfg = get_simple_config(dialect.into(), None, exclude_rules, config_path)?;

    let mut linter = Linter::new(cfg, formatter, None, false);

    let mut result = linter.lint_string_wrapped(sql, None, false);

    Ok(take(&mut result.paths[0].files[0].violations))
}

pub fn fix(sql: &str) -> String {
    let cfg = get_simple_config(Some("ansi".into()), None, None, None).unwrap();
    let mut linter = Linter::new(cfg, None, None, false);
    let mut result = linter.lint_string_wrapped(sql, None, true);
    take(&mut result.paths[0].files[0]).fix_string()
}
