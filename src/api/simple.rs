use crate::core::config::FluffConfig;
use crate::core::dialects::init::dialect_selector;
use crate::core::errors::SQLFluffUserError;
use std::collections::HashMap;

// fn lint(sql: String, dialect: String, rules: Option<Vec<String>>, exclude_rules: Option<Vec<String>>, config_path: Option<String>) -> String {
//
// }

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
