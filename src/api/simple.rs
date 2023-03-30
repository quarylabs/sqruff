use crate::core::config::FluffConfig;
use crate::core::dialects::init::dialect_selector;
use crate::core::errors::SQLFluffUserError;
use crate::core::linter::linter::Linter;
use std::collections::HashMap;

pub fn get_simple_config(
    dialect: Option<String>,
    rules: Option<Vec<String>>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<FluffConfig, SQLFluffUserError> {
    let mut overrides = HashMap::new();
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
    let out = FluffConfig::from_root(config_path, true, Some(overrides))
        .map_err(|err| SQLFluffUserError::new(format!("Error loading config: {:?}", err)));
    return out;
}

/// Lint a SQL string.
pub fn lint(
    sql: String,
    dialect: String,
    rules: Option<Vec<String>>,
    exclude_rules: Option<Vec<String>>,
    config_path: Option<String>,
) -> Result<String, SQLFluffUserError> {
    let cfg = get_simple_config(Some(dialect), rules, exclude_rules, config_path)?;
    let linter = Linter::new(cfg, None);
    let result = linter.lint_string_wrapped(sql, None, None);
    panic!("Not implemented");
    // let result_records = result.as_records();
    // // Return just the violations for this file
    // return [] if not result_records else result_records[0]["violations"]
}
