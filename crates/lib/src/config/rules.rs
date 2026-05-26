use hashbrown::HashMap;
use serde::Deserialize;

use super::de;
use super::error::ConfigError;
use super::raw::{RawConfig, Value};
use super::setting::Merge;

const KNOWN_RULE_OPTIONS: &[&str] = &[
    "additional_allowed_characters",
    "alias_case_check",
    "aliasing",
    "allow_scalar",
    "allow_space_in_identifier",
    "blocked_regex",
    "blocked_words",
    "capitalisation_policy",
    "extended_capitalisation_policy",
    "forbid_subquery_in",
    "force_enable",
    "fully_qualify_join_types",
    "group_by_and_order_by_style",
    "ignore_comment_clauses",
    "ignore_comment_lines",
    "ignore_words",
    "ignore_words_regex",
    "match_source",
    "max_alias_length",
    "maximum_empty_lines_between_statements",
    "maximum_empty_lines_inside_statements",
    "min_alias_length",
    "multiline_newline",
    "prefer_count_0",
    "prefer_count_1",
    "prefer_quoted_identifiers",
    "prefer_quoted_keywords",
    "preferred_first_table_in_join_clause",
    "preferred_not_equal_style",
    "preferred_quoted_literal_style",
    "preferred_type_casting_style",
    "quoted_identifiers_policy",
    "require_final_semicolon",
    "select_clause_trailing_comma",
    "single_table_references",
    "unquoted_identifiers_policy",
    "wildcard_policy",
];

/// Typed patch for the `[sqruff:rules]` section and per-rule subsections.
///
/// Unknown keys are captured by the flattened map, allowing arbitrary
/// rule-specific overrides (e.g. `[sqruff:rules:LT01]`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuleConfigsPatch {
    #[serde(flatten)]
    pub configs: HashMap<String, Value>,
}

impl RuleConfigsPatch {
    pub(crate) fn merge_global(
        &mut self,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        self.configs
            .extend(de::deserialize_value_map(section_name, values)?);
        Ok(())
    }

    pub(crate) fn merge_rule_section(
        &mut self,
        rule_section: String,
        section_name: &str,
        values: &std::collections::HashMap<String, Option<String>>,
    ) -> Result<(), ConfigError> {
        validate_rule_options(section_name, values)?;
        self.configs.insert(
            rule_section,
            Value::Map(de::deserialize_value_map(section_name, values)?),
        );
        Ok(())
    }

    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.configs.is_empty() {
            return;
        }
        let rules = raw
            .entry("rules".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let rules_map = rules.as_map_mut().expect("rules must be a map");
        rules_map.extend(self.configs);
    }
}

fn validate_rule_options(
    section_name: &str,
    values: &std::collections::HashMap<String, Option<String>>,
) -> Result<(), ConfigError> {
    for key in values.keys() {
        if !KNOWN_RULE_OPTIONS.contains(&key.as_str()) {
            return Err(ConfigError::InvalidSection {
                section: section_name.to_string(),
                reason: format!("invalid rule option '{key}'"),
            });
        }
    }
    Ok(())
}

impl Merge for RuleConfigsPatch {
    fn merge(&mut self, other: Self) {
        self.configs.extend(other.configs);
    }
}

/// Resolved rule configurations.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleConfigs {
    pub(super) values: HashMap<String, Value>,
}

impl RuleConfigs {
    pub(super) fn from_raw(raw: &RawConfig) -> Self {
        Self {
            values: raw["rules"].as_map().unwrap().clone(),
        }
    }

    pub fn config_map(&self, rule_config_ref: &str) -> HashMap<String, Value> {
        if rule_config_ref.is_empty() || rule_config_ref == "rules" {
            return self.values.clone();
        }

        // Start with scalar values from the global [rules] section
        let mut merged: HashMap<String, Value> = self
            .values
            .iter()
            .filter(|(_, v)| !matches!(v, Value::Map(_)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Override/extend with rule-specific values
        if let Some(specific) = self.values.get(rule_config_ref).and_then(Value::as_map) {
            merged.extend(specific.clone());
        }

        merged
    }
}
