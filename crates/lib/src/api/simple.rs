use std::mem::take;

use ahash::AHashMap;

use crate::cli::formatters::OutputStreamFormatter;
use crate::core::config::FluffConfig;
use crate::core::dialects::init::dialect_selector;
use crate::core::errors::{SQLBaseError, SQLFluffUserError};
use crate::core::linter::linter::Linter;
use crate::core::rules::base::ErasedRule;

pub fn get_simple_config(
    dialect: Option<String>,
    rules: Option<Vec<String>>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<FluffConfig, SQLFluffUserError> {
    let mut overrides = AHashMap::new();
    if let Some(dialect) = dialect {
        let selected = dialect_selector(&dialect);
        if selected.is_none() {
            return Err(SQLFluffUserError::new(format!(
                "Error loading dialect '{}': {}",
                dialect, "Dialect not found"
            )));
        }
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
    rules: Vec<ErasedRule>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<Vec<SQLBaseError>, SQLFluffUserError> {
    lint_with_formatter(sql, dialect, rules, exclude_rules, config_path, None)
}

/// Lint a SQL string.
pub fn lint_with_formatter(
    sql: String,
    dialect: String,
    rules: Vec<ErasedRule>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
    formatter: Option<OutputStreamFormatter>,
) -> Result<Vec<SQLBaseError>, SQLFluffUserError> {
    let cfg = get_simple_config(dialect.into(), None, exclude_rules, config_path)?;

    let mut linter = Linter::new(cfg, None, None);
    linter.formatter = formatter;

    let mut result = linter.lint_string_wrapped(sql, None, None, rules);

    Ok(take(&mut result.paths[0].files[0].violations))
}

pub fn fix(sql: String, rules: Vec<ErasedRule>) -> String {
    let cfg = get_simple_config(Some("ansi".into()), None, None, None).unwrap();
    let mut linter = Linter::new(cfg, None, None);
    let result = linter.lint_string_wrapped(sql, None, Some(true), rules);
    result.paths[0].files[0].fix_string()
}
