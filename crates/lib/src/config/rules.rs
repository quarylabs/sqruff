use hashbrown::HashMap;
use serde::Deserialize;

use super::raw::{RawConfig, Value};

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
