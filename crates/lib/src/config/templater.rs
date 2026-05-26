use hashbrown::HashMap;
use serde::Deserialize;

use super::raw::{RawConfig, Value};
use crate::templaters::TemplaterKind;

/// Typed patch for `[sqruff:templater:<name>]` sections.
///
/// Each key in the flattened map corresponds to a templater name (e.g.
/// `"placeholder"`, `"jinja"`), and the associated value is the per-templater
/// configuration map.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TemplaterConfigPatch {
    #[serde(flatten)]
    pub templaters: HashMap<String, Value>,
}

impl TemplaterConfigPatch {
    pub(super) fn merge_into_raw(self, raw: &mut RawConfig) {
        if self.templaters.is_empty() {
            return;
        }
        let templater = raw
            .entry("templater".into())
            .or_insert_with(|| Value::Map(HashMap::new()));
        let templater_map = templater.as_map_mut().expect("templater must be a map");
        templater_map.extend(self.templaters);
    }
}

/// Resolved templater configuration, including per-templater sub-sections.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplaterConfig {
    pub(super) values: HashMap<String, Value>,
    pub(super) kind: TemplaterKind,
}

impl TemplaterConfig {
    pub(super) fn from_raw(raw: &RawConfig) -> Self {
        let kind = raw["core"]["templater"]
            .as_string()
            .map(TemplaterKind::from_name)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or(TemplaterKind::Raw);

        Self {
            values: raw["templater"].as_map().cloned().unwrap_or_default(),
            kind,
        }
    }

    pub fn kind(&self) -> TemplaterKind {
        self.kind
    }

    pub fn section(&self, templater: TemplaterKind) -> Option<&HashMap<String, Value>> {
        self.values.get(templater.as_str()).and_then(Value::as_map)
    }

    pub fn value(&self, templater: TemplaterKind, key: &str) -> Option<&Value> {
        self.section(templater)?.get(key)
    }

    #[cfg(feature = "python")]
    pub(crate) fn root_value(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }
}
